use std::cell::RefCell;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use itertools::Itertools;
use twenty_first::shared_math::circuit::{
    BinOp, Circuit, CircuitBuilder, CircuitExpression, CircuitMonad,
};
use twenty_first::shared_math::mds::recursive_cyclic_mul;

#[allow(dead_code)]
fn build_recursive_cyclic_mul_circuit() -> [Circuit<u64>; 16] {
    const STATE_SIZE: usize = 16;
    const MDS_MATRIX_FIRST_COLUMN: [u64; STATE_SIZE] = [
        61402, 1108, 28750, 33823, 7454, 43244, 53865, 12034, 56951, 27521, 41351, 40901, 12021,
        59689, 26798, 17845,
    ];
    type T = u64;
    let builder = CircuitBuilder::<T>::new();

    let inputs: [CircuitMonad<T>; STATE_SIZE] = (0..STATE_SIZE)
        .map(|i| builder.input(i))
        .collect_vec()
        .try_into()
        .unwrap();
    let mds_column = MDS_MATRIX_FIRST_COLUMN.map(|c| builder.constant(c));
    let mut outputs = recursive_cyclic_mul(
        &inputs,
        &mds_column,
        STATE_SIZE,
        builder.constant(0),
        builder.constant(2),
    );

    CircuitMonad::distribute_constants(&mut outputs);
    CircuitMonad::constant_folding(&mut outputs);
    CircuitMonad::move_coefficients_right(&mut outputs);
    CircuitMonad::fold_uncles(&mut outputs);

    outputs
        .into_iter()
        .map(|o| o.consume())
        .collect_vec()
        .try_into()
        .unwrap()
}

fn fmt_node_id(id: usize) -> String {
    format!("n_{id}")
}
fn fmt_input(i: usize) -> String {
    format!("(input[{i}] as u64)")
}

/// Count *uses by parents* for each node across all requested outputs.
/// This is *edge multiplicity*: if a node feeds 3 parents (possibly via two different outputs), count = 3.
fn compute_use_counts(outputs: &[Rc<RefCell<Circuit<u64>>>]) -> BTreeMap<usize, usize> {
    let mut uses: BTreeMap<usize, usize> = BTreeMap::new();
    // Worklist with multiplicity of demand (here 1 per appearance is fine).
    let mut stack: Vec<Rc<RefCell<Circuit<u64>>>> = outputs.to_vec();

    while let Some(nrc) = stack.pop() {
        let n = nrc.borrow();
        match &n.expression {
            CircuitExpression::BinaryOperation(_, a, b) => {
                *uses.entry(a.borrow().id).or_insert_with(|| {
                    stack.push(a.clone());
                    0
                }) += 1;
                *uses.entry(b.borrow().id).or_insert_with(|| {
                    stack.push(b.clone());
                    0
                }) += 1;
            }
            CircuitExpression::Input(_) | CircuitExpression::Constant(_) => {
                uses.remove(&n.id);
            }
        }
    }
    uses
}

fn expand_expr(e: &Circuit<u64>, uses: &BTreeMap<usize, usize>) -> String {
    match &e.expression {
        CircuitExpression::BinaryOperation(o, a, b) => {
            let a = a.borrow();
            let b = b.borrow();
            let a = if uses.contains_key(&a.id) {
                fmt_node_id(a.id)
            } else {
                expand_expr(&a, uses)
            };
            let b = if uses.contains_key(&b.id) {
                fmt_node_id(b.id)
            } else {
                expand_expr(&b, uses)
            };
            let o = match o {
                BinOp::Add => "wrapping_add",
                BinOp::Sub => "wrapping_sub",
                BinOp::Mul => "wrapping_mul",
            };
            format!("{a}.{o}({b})")
        }
        CircuitExpression::Input(i) => fmt_input(*i),
        CircuitExpression::Constant(c) => {
            format!("0x{c:x}u64")
        }
    }
}

struct DeclLayer {
    exprs: Vec<Rc<RefCell<Circuit<u64>>>>,
}

impl DeclLayer {
    pub fn spit_code(&self, uses: &BTreeMap<usize, usize>, indent: &str) -> Vec<String> {
        let mut out = vec![];
        for e in &self.exprs {
            let e = e.borrow();
            out.push(format!(
                "{indent}let {} = {};",
                fmt_node_id(e.id),
                expand_expr(&e, uses)
            ));
        }
        out
    }
}

struct Compiler {
    uses: BTreeMap<usize, usize>,
    out: Vec<Rc<RefCell<Circuit<u64>>>>,
    layers: Vec<DeclLayer>,
}

impl Compiler {
    pub fn new(out: Vec<Rc<RefCell<Circuit<u64>>>>) -> Self {
        Self {
            uses: compute_use_counts(&out),
            out,
            layers: vec![],
        }
    }

    pub fn build_layers(&mut self) {
        let mut stack = self.out.clone();
        let mut consumed = BTreeSet::new();
        loop {
            let mut next_stack = vec![];
            while let Some(erc) = stack.pop() {
                let e = erc.borrow();
                match self.uses.entry(e.id) {
                    Entry::Occupied(mut entry) => {
                        let entry = entry.get_mut();
                        if *entry == 0 {
                            assert!(consumed.insert(e.id), "{}", e.id);
                            if let CircuitExpression::BinaryOperation(_, a, b) = &e.expression {
                                stack.push(a.clone());
                                stack.push(b.clone());
                            }
                        } else {
                            if *entry == 1 {
                                next_stack.push(erc.clone());
                            }
                            *entry -= 1;
                        }
                    }
                    Entry::Vacant(_) => {
                        if let CircuitExpression::BinaryOperation(_, a, b) = &e.expression {
                            stack.push(a.clone());
                            stack.push(b.clone());
                        }
                    }
                }
            }
            if !next_stack.is_empty() {
                // Sort layer by ID, because that seems to give nice memory layout
                next_stack.sort_by_key(|v| v.borrow().id);
                self.layers.push(DeclLayer {
                    exprs: next_stack.clone(),
                });
                core::mem::swap(&mut stack, &mut next_stack);
            } else {
                break;
            }
        }
        for (k, v) in self.uses.iter() {
            assert_eq!(*v, 0, "{k} uses not 0");
        }
    }

    pub fn spit_code(&mut self) -> String {
        let mut lines = vec![
            "// generated with mds-codegen".to_string(),
            "pub const fn generated(input: &[u32; 16]) -> [u64; 16] {".to_string(),
        ];

        for (i, layer) in self.layers.iter().rev().enumerate() {
            lines.push(format!("    // layer {i}"));
            lines.extend(layer.spit_code(&self.uses, "    "));
        }

        lines.push("    // output".to_string());
        lines.push("    [".to_string());
        for e in &self.out {
            lines.push(format!("        {},", expand_expr(&e.borrow(), &self.uses)));
        }
        lines.push("    ]".to_string());

        lines.push("}".to_string());
        lines.join("\n")
    }
}

fn fold_identical_exprs(outputs: &[Rc<RefCell<Circuit<u64>>>]) {
    loop {
        let mut visited = BTreeSet::new();
        let mut expr_map = HashMap::new();
        let mut stack = outputs.to_vec();
        let mut exprs_hit = 0;
        while let Some(erc) = stack.pop() {
            let e = erc.borrow();
            if !visited.insert(e.id) {
                continue;
            }
            exprs_hit += 1;
            if let CircuitExpression::BinaryOperation(o, a, b) = &e.expression {
                expr_map
                    .entry((*o, a.borrow().id, b.borrow().id))
                    .or_insert(vec![])
                    .push(erc.clone());
                stack.push(a.clone());
                stack.push(b.clone());
            }
        }
        let mut folded_count = 0;
        for (_, v) in expr_map {
            if v.len() > 1 {
                let first = v[0].borrow().id;
                v.iter().for_each(|v| v.borrow_mut().id = first);
                folded_count += 1;
            }
        }
        eprintln!("Folded from {exprs_hit} - {folded_count} exprs");
        if folded_count == 0 {
            break;
        }
    }
}

pub fn spit_code(outputs: &[Circuit<u64>]) {
    let outputs: Vec<Rc<RefCell<Circuit<u64>>>> = outputs
        .iter()
        .map(|v| Rc::new(RefCell::new(v.clone())))
        .collect();

    fold_identical_exprs(&outputs);

    let mut compiler = Compiler::new(outputs);
    compiler.build_layers();
    println!("{}", compiler.spit_code());
}

fn main() {
    let circuit = build_recursive_cyclic_mul_circuit();
    spit_code(&circuit[..]);
}
