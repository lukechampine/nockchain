use std::cell::RefCell;
use std::rc::Rc;

use itertools::Itertools;
use twenty_first::shared_math::circuit::{
    self, Circuit, CircuitBuilder, CircuitExpression, CircuitMonad,
};
use twenty_first::shared_math::mds::recursive_cyclic_mul;
use twenty_first::shared_math::tip5::STATE_SIZE;

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

fn spit_code(c: &[Circuit<u64>]) {
    fn fmt_node(id: usize) -> String {
        format!("n_{id}")
    }

    // start stack with all outputs
    let mut stack: Vec<(Rc<RefCell<Circuit<u64>>>, bool)> = c
        .iter()
        .map(|v| Rc::new(RefCell::new(v.clone())))
        .map(|n| (n, false)) // false = not yet expanded
        .collect();

    let mut emitted = std::collections::BTreeSet::new();
    let mut out: Vec<String> = Vec::new();

    while let Some((node, expanded)) = stack.pop() {
        let id = node.borrow().id;

        // if we're seeing it before expansion, push a finish marker then its deps
        if !expanded {
            if emitted.contains(&id) {
                continue;
            } // already fully handled
            stack.push((node.clone(), true)); // finish marker
            match &node.borrow().expression {
                CircuitExpression::BinaryOperation(_, a, b) => {
                    stack.push((a.clone(), false));
                    stack.push((b.clone(), false));
                }
                CircuitExpression::Input(_) | CircuitExpression::Constant(_) => {}
            }
            continue;
        }

        // finish time: children have been handled; emit now (once)
        if !emitted.insert(id) {
            continue;
        }

        let rhs = match &node.borrow().expression {
            CircuitExpression::Input(i) => format!("input[{i}] as u64"),
            CircuitExpression::Constant(c) => format!("0x{c:x}u64"),
            CircuitExpression::BinaryOperation(op, a, b) => {
                let a = fmt_node(a.borrow().id);
                let b = fmt_node(b.borrow().id);
                match op {
                    circuit::BinOp::Add => format!("{a}.wrapping_add({b})"),
                    circuit::BinOp::Sub => format!("{a}.wrapping_sub({b})"),
                    circuit::BinOp::Mul => format!("{a}.wrapping_mul({b})"),
                }
            }
        };
        out.push(format!("    let {} = {rhs};", fmt_node(id)));
    }

    // final value vector (in the order of c)
    out.push(format!(
        "    [{}]",
        c.iter()
            .map(|v| fmt_node(v.id))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    println!("// generated with mds-codegen\npub const fn generated(input: &[u32; 16]) -> [u64; 16] {{\n{}\n}}", out.join("\n"));
}

fn main() {
    let circuit = build_recursive_cyclic_mul_circuit();
    spit_code(&circuit[..]);
}
