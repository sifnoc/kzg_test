#![feature(generic_const_exprs)]
#![feature(int_log)]
mod user_data;
use kzg_mst::circuits::kzg::KZGCircuitParams;
use user_data::UserData;

use ark_std::{end_timer, start_timer};
use halo2_base::halo2_proofs::halo2curves::bn256::{Bn256, Fr};
use halo2_base::halo2_proofs::plonk::{create_proof, keygen_pk, keygen_vk};
use halo2_base::halo2_proofs::poly::kzg::commitment::KZGCommitmentScheme;
use halo2_base::halo2_proofs::poly::kzg::multiopen::ProverGWC;
use halo2_base::halo2_proofs::transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer};
use halo2_base::utils::fs::gen_srs;
use halo2_base::{
    gates::builder::{
        CircuitBuilderStage, GateThreadBuilder, MultiPhaseThreadBreakPoints, RangeCircuitBuilder,
    },
    halo2_proofs::halo2curves::{bn256::G1Affine, FieldExt},
};
use halo2_ecc::{commitments::utils::blob::root_of_unity, fields::FpStrategy};
use rand_core::OsRng;

use kzg_mst::circuits::kzg::kzg_multi_test;
use summa_solvency::merkle_sum_tree::utils::parse_csv_to_entries;

fn kzg_circuit<const N_ASSETS: usize>(
    params: KZGCircuitParams,
    stage: CircuitBuilderStage,
    break_points: Option<MultiPhaseThreadBreakPoints>,
    csv_path: &str,
    user_index: usize,
) -> RangeCircuitBuilder<Fr>
where
    [usize; N_ASSETS + 1]: Sized,
{
    let entries = parse_csv_to_entries::<&str, N_ASSETS, 8>(csv_path).unwrap();
    let entries_len = entries.len();

    // temporary values for in-secure trust setup
    let tau: Fr = Fr::from(111);
    let kzg_k: u32 = entries_len.ilog2() + 3;

    let from = user_index * N_ASSETS + 1;
    let to = from + N_ASSETS + 1;

    let pp = UserData::<N_ASSETS>::mock_trusted_setup(tau, (1 << kzg_k) + 1, (to * 3) + 2);

    // Load user data from the entry
    let user_data = UserData::new(entries, pp.clone(), kzg_k);

    let p_bar = user_data.commit_vector();
    let q_bar = user_data.open_prf(from, to);

    let selected_root = root_of_unity(kzg_k as u32);

    let open_idxs: Vec<Fr> = (from..=to)
        .map(|op| selected_root.pow(&[op.clone() as u64, 0, 0, 0]))
        .collect();
    let open_vals: Vec<Fr> = (from..=to)
        .map(|op| user_data.data[op.clone() as usize])
        .collect();
    println!("open_idx: {:?}", open_idxs);
    println!("open_vals:{:?}", open_vals);

    let k: usize = params.degree as usize;
    let mut builder = match stage {
        CircuitBuilderStage::Mock => GateThreadBuilder::mock(),
        CircuitBuilderStage::Prover => GateThreadBuilder::prover(),
        CircuitBuilderStage::Keygen => GateThreadBuilder::keygen(),
    };

    kzg_multi_test(
        &mut builder,
        params,
        p_bar,
        open_idxs,
        open_vals,
        q_bar,
        pp.ptau_g1[..to].to_vec(),
        pp.ptau_g2[..=to].to_vec(),
    );

    let circuit = match stage {
        CircuitBuilderStage::Mock => {
            builder.config(k, Some(20));
            RangeCircuitBuilder::mock(builder)
        }
        CircuitBuilderStage::Keygen => {
            builder.config(k, Some(20));
            RangeCircuitBuilder::keygen(builder)
        }
        CircuitBuilderStage::Prover => RangeCircuitBuilder::prover(builder, break_points.unwrap()),
    };

    circuit
}

fn main() {
    // This code comes from `random_kzg_multi_circuit` method in https://github.com/punwai/halo2-lib/tree/kzg
    const N_ASSETS: usize = 2;

    // {"strategy":"Simple","degree":17,"num_advice":6,"num_lookup_advice":1,"num_fixed":1,"lookup_bits":8,"limb_bits":90,"num_limbs":3}
    let params = KZGCircuitParams::new(FpStrategy::Simple, 19, 6, 1, 1, 18, 90, 3);

    let circuit = kzg_circuit::<N_ASSETS>(
        params,
        CircuitBuilderStage::Mock,
        None,
        "./src/entry_16.csv",
        0,
    );

    // Mocking test
    // MockProver::run(params.degree, &circuit, vec![])
    //     .unwrap()
    //     .assert_satisfied();

    let params = gen_srs(19);

    let vk_time = start_timer!(|| "Generating vkey");
    let vk = keygen_vk(&params, &circuit).unwrap();
    end_timer!(vk_time);

    let pk_time = start_timer!(|| "Generating pkey");
    let pk = keygen_pk(&params, vk, &circuit).unwrap();
    end_timer!(pk_time);

    let proof_time = start_timer!(|| "Proving time");
    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
    create_proof::<
        KZGCommitmentScheme<Bn256>,
        ProverGWC<'_, Bn256>,
        Challenge255<G1Affine>,
        _,
        Blake2bWrite<Vec<u8>, G1Affine, Challenge255<G1Affine>>,
        _,
    >(&params, &pk, &[circuit], &[&[]], OsRng, &mut transcript)
    .unwrap();
    let proof = transcript.finalize();
    end_timer!(proof_time);

    // proof size
    println!("proof size: {}", proof.len());
}