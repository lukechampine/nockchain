use std::sync::{Arc, Mutex as SyncMutex};
use std::time::Instant;

use kernels::miner::KERNEL;
use nbx_jetpack::instruments::{local_instruments, Instruments, ReadInstruments};
use nbx_jetpack::log::*;
use nockapp::kernel::form::SerfThread;
use nockapp::noun::slab::NounSlab;
use nockapp::save::SaveableCheckpoint;
use nockapp::utils::NOCK_STACK_SIZE_TINY;
use nockapp::wire::WireRepr;
use nockapp::CrownError;
use nockvm::interpreter::NockCancelToken;
use nockvm::jets::hot::HotEntry;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::metrics::{counter, histogram};

pub struct PokerAttemptRes<M> {
    pub id: usize,
    pub duration_millis: u32,
    pub inst_delta: ReadInstruments,
    pub slab_res: Result<NounSlab, CrownError>,
    pub slab_inp: NounSlab,
    pub metadata: M,
}

struct Poker<M> {
    serf: SerfThread<SaveableCheckpoint, rayon::ThreadPool>,
    id: usize,
    results: mpsc::Sender<PokerAttemptRes<M>>,
    reqs: watch::Receiver<SyncMutex<Option<(NounSlab, M)>>>,
    instruments: Arc<Instruments>,
    poke_wire: WireRepr,
}

impl<M: Send + 'static> Poker<M> {
    pub async fn run(mut self) {
        let mut prev_inst = ReadInstruments::default();

        while self.reqs.changed().await.is_ok() {
            let Some((poke_slab, metadata)) = ({
                let mtx = self.reqs.borrow_and_update();
                let mut guard = mtx.lock().expect("Poisoned lock");
                guard.take()
            }) else {
                continue;
            };

            let attempts_counter = counter!(
                "nbx_miner_client_attempts_count",
                "poker_id" => self.id.to_string(),
            );
            attempts_counter.increment(1);

            let start = Instant::now();
            let result = self
                .serf
                .poke(self.poke_wire.clone(), poke_slab.clone())
                .await;

            let cur_inst = self.instruments.read();
            let inst_delta = cur_inst.since(prev_inst);
            prev_inst = cur_inst;

            let duration_millis = start.elapsed().as_millis() as u32;

            trace!("duration_millis: {duration_millis}\ninstrumentation: {inst_delta:#?}",);

            let results = PokerAttemptRes {
                duration_millis,
                inst_delta,
                id: self.id,
                slab_res: result,
                slab_inp: poke_slab,
                metadata,
            };

            let attempt_hist = histogram!(
                "nbx_miner_client_attempt_seconds",
                "poker_id" => self.id.to_string(),
            );
            attempt_hist.record((results.duration_millis as f64) / 1000.0);

            if self.results.send(results).await.is_err() {
                break;
            }
        }
    }
}

pub struct PokerHandle<M> {
    reqs: watch::Sender<SyncMutex<Option<(NounSlab, M)>>>,
    reqs_rx: watch::Receiver<SyncMutex<Option<(NounSlab, M)>>>,
    cancellation: NockCancelToken,
    poker_loop: JoinHandle<()>,
    id: usize,
}

impl<M: Send + 'static> PokerHandle<M> {
    pub async fn new(
        hot_state: Vec<HotEntry>,
        test_jets: Vec<NounSlab>,
        id: usize,
        thread_pin: Option<usize>,
        results: mpsc::Sender<PokerAttemptRes<M>>,
        poke_wire: WireRepr,
    ) -> Self {
        let kernel = Vec::from(KERNEL);
        let serf = SerfThread::<SaveableCheckpoint, rayon::ThreadPool>::new(
            kernel,
            None,
            hot_state,
            NOCK_STACK_SIZE_TINY,
            test_jets,
            Default::default(),
            false,
        )
        .await
        .expect("Could not load mining kernel");

        let cancellation = serf.cancel_token.clone();

        if let Some(core_id) = thread_pin {
            debug!("Pinning poker {id} to core {core_id}");
            serf.call_fn(move || gdt_cpus::pin_thread_to_core(core_id))
                .await
                .expect("Could not invoke core pinning")
                .expect("Could not pin the poker thread");
        }

        let instruments = serf
            .call_fn(local_instruments)
            .await
            .expect("Unable to get instruments");

        let (tx, rx) = watch::channel(SyncMutex::new(None));

        let poker = Poker {
            serf,
            id,
            results,
            reqs: rx.clone(),
            instruments,
            poke_wire,
        };

        let poker_loop = tokio::spawn(poker.run());

        Self {
            reqs: tx,
            reqs_rx: rx,
            cancellation,
            poker_loop,
            id,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub async fn await_free_request(&self) {
        self.reqs_rx
            .clone()
            .wait_for(|v| v.lock().unwrap().is_some())
            .await;
    }

    pub fn send_poke(&self, poke_slab: NounSlab, metadata: M, cancel_previous: bool) {
        self.reqs.send_modify(|v| {
            let mut guard = v.lock().expect("Poisoned lock");
            *guard = Some((poke_slab, metadata));
            if cancel_previous {
                // Cancel while holding the guard to prevent the poker from racing to a stale request.
                self.cancel_current_poke();
            }
        });
    }

    pub fn cancel_current_poke(&self) {
        self.cancellation.cancel();
    }

    pub async fn finish(self) {
        self.cancel_current_poke();
        core::mem::drop(self.reqs_rx);
        core::mem::drop(self.reqs);
        let _ = self.poker_loop.await;
    }
}
