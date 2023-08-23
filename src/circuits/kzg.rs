/*
 * Test suite for KZGChip.
 */
use halo2_ecc::{
    bn254::{pairing::PairingChip, Fp2Chip, FpChip},
    commitments::{kzg::KZGChip, FrChip},
    ecc::EccChip,
    fields::{poly::PolyChip, FieldChip, FpStrategy},
    halo2_base::{
        gates::{builder::GateThreadBuilder, RangeChip},
        halo2_proofs::halo2curves::bn256::{Fr as Fp, G1Affine, G2Affine, G1, G2},
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct KZGCircuitParams {
    strategy: FpStrategy,
    pub degree: u32,
    num_advice: usize,
    num_lookup_advice: usize,
    num_fixed: usize,
    lookup_bits: usize,
    limb_bits: usize,
    num_limbs: usize,
}

impl KZGCircuitParams {
    pub fn new(
        strategy: FpStrategy,
        degree: u32,
        num_advice: usize,
        num_lookup_advice: usize,
        num_fixed: usize,
        lookup_bits: usize,
        limb_bits: usize,
        num_limbs: usize,
    ) -> Self {
        KZGCircuitParams {
            strategy,
            degree,
            num_advice,
            num_lookup_advice,
            num_fixed,
            lookup_bits,
            limb_bits,
            num_limbs,
        }
    }
}

/*
 * Assigns all input values for the KZGChip and proves multi-opens.
 */
pub fn kzg_multi_test(
    builder: &mut GateThreadBuilder<Fp>,
    params: KZGCircuitParams,
    p_bar: G1Affine,
    open_idxs: Vec<Fp>,
    open_vals: Vec<Fp>,
    q_bar: G1Affine,
    ptau_g1: Vec<G1>,
    ptau_g2: Vec<G2>,
) {
    let ctx = builder.main(0);
    std::env::set_var("LOOKUP_BITS", params.lookup_bits.to_string());

    // Initialize chips
    let range = RangeChip::<Fp>::default(params.lookup_bits);
    let fr_chip = FrChip::<Fp>::new(&range, params.limb_bits, params.num_limbs);
    let fp_chip = FpChip::<Fp>::new(&range, params.limb_bits, params.num_limbs);
    let g1_chip = EccChip::new(&fp_chip);
    let fp2_chip = Fp2Chip::<Fp>::new(&fp_chip);
    let g2_chip = EccChip::new(&fp2_chip);
    let pairing_chip = PairingChip::new(&fp_chip);
    let poly_chip = PolyChip::new(&fr_chip);

    // Load individual group elements
    let assigned_q_bar = g1_chip.assign_point(ctx, q_bar);
    let assigned_p_bar = g1_chip.assign_point(ctx, p_bar);

    // Load vectors
    let ptau_g1_loaded = ptau_g1
        .iter()
        .map(|x| g1_chip.assign_point(ctx, G1Affine::from(x)))
        .collect::<Vec<_>>();
    let ptau_g2_loaded = ptau_g2
        .iter()
        .map(|x| g2_chip.assign_point(ctx, G2Affine::from(x)))
        .collect::<Vec<_>>();

    let mut load_fr = |x: Vec<Fp>| {
        x.into_iter()
            .map(|c| fr_chip.load_private(ctx, c))
            .collect::<Vec<_>>()
    };
    let open_idxs_loaded = load_fr(open_idxs);
    let open_vals_loaded = load_fr(open_vals);

    // Test chip
    let kzg_chip = KZGChip::new(&poly_chip, &pairing_chip, &g1_chip, &g2_chip);
    kzg_chip.opening_assert(
        builder,
        assigned_p_bar,
        &open_idxs_loaded,
        &open_vals_loaded,
        assigned_q_bar,
        &ptau_g1_loaded[..],
        &ptau_g2_loaded[..],
    );
}
