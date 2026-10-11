pub fn tick(vel: Vec3, rot: Rot) -> Vec3 {
    // ... asserts, unpacking ...

    let pitch_rad = pitch * (std::f64::consts::PI / 180.0);
    let pitch_cos = pitch_rad.cos();

    // q = pitch_cos2: 1 at pitch 0, ~0 at pitch 85
    let q = pitch_cos * pitch_cos;

    // TODO: why does chatter grow from the end of the dive?

    // let t be the current tick, so q_t = pitch_cos2 (= q?).
    // let dq_t > 0.0 be small. (TODO: should this be > 0.0?)
    // let dq_{t+1} = -dq_t.
    // consider perturbing q_t += dq_t, q_{t+1} -= dq_t.
    // we're interested in whether this gains utility,
    // ie whether ΔU > 0.0.
    // (or rather, *why* it gains utility)
    // (as well as how much utility it gains)
    // if it does gains utility,
    // that means the optimizer is pushed away from smooth flight.

    // TODO: why are we using a second order expansion and not a first order expansion?
    // TODO: why does it have period 2

    // notation:
    // U_t = ∂U/∂q_t
    // U_tt = ∂²U/∂q_t²
    // U_t,t+1 = ∂²U/(∂q_t ∂q_{t+1})

    // second order taylor expansion:
    // ΔU ≈ U_t·dq_t + U_{t+1}·dq_{t+1} + ½·(U_tt·dq_t² + 2·U_t,t+1·dq_t·dq_{t+1} + U_{t+1,t+1}·dq_{t+1}²)

    // our perturbation was symmetric; dq_{t+1} = −dq_t:
    // ΔU ≈ (U_t − U_{t+1})·dq_t + ½·(U_tt − 2·U_t,t+1 + U_{t+1,t+1})·dq_t²

    // // approximate U_{t+1,t+1} ≈ U_tt (TODO: why is this fine)
    // // ΔU ≈ (U_t − U_{t+1})·dq_t + (U_tt − U_t,t+1)·dq_t²

    // // approximate U_{t+1} ≈ U_t (TODO: why is this fine)
    // // ΔU ≈ (U_tt − U_t,t+1)·dq_t²


    // suppose we're in smooth flight, so u_t ≈ u_{t+1}.

    // The gradient term is about zero:
    // an optimum or a smooth path has no first-order reason to prefer +d over -d.
    // Neighbors are nearly identical,
    // so `U_{t+1,t+1} ≈ U_tt`.
    // That leaves `ΔU ≈ dq_t²·(U_tt - U_t,t+1)`.

    // So the sign of `U_tt - U_t,t+1` decides it:

    // **Negative:** smooth is locally a maximum along the alternating direction,
    // and the optimizer stays smooth.

    // **Positive:** smooth is a saddle along that direction.
    // Any wiggle grows until it hits the bounds on u.
    // The bounds are 1 at pitch 0 and cos²85° = 0.0076 at the cap,
    // which is why the chatter is 85/0 and not a gentle wobble.

    // In words: U_tt pushes you to spread lift out evenly.
    // A negative U_t,t+1 rewards concentrating it,
    // because the ticks compete for the same sink.
    // Chatter appears when competition beats diminishing returns.

    // **U_tt:** how the marginal utility of q at tick t
    // changes as q at that tick goes up.
    // It's negative because more q shrinks the sink
    // that q's own conversion acts on.

    // **U_t,t+1:** how q at tick t changes
    // the marginal utility of q at tick t+1.

    // we're considering taking derivatives wrt q
    // (while holding vy0 constant),
    // so phrase vy_grav as affine in q.
    // vy_grav = A + B*q, where
    //   A = vy0 - g        what vy would be with no lift
    //   B = 0.75*g = 0.06  lift per unit of q
    const B: f64 = 0.75 * GRAVITY;
    let vy_grav = (vy0 - GRAVITY) + B * q;

    // convert = -0.1 * q * (A + B*q)
    //   more q gives less convert per unit of q (concave in q):
    //     d(convert)/dq = -0.1*A - 0.2*B*q
    //       >= 0.0 when q <= -A * (25.0 / 3.0)    (which is most of the time)
    //     d²convert/dq² = -0.2*B = -0.012
    //   => U_tt = -0.2*B*k1    (diminishing return within a tick)
    let convert = -0.1 * q * vy_grav;

    // vy_fall = (A + B*q)(1 - 0.1*q)
    //   d(vy_fall)/dq = B - 0.1*A - 0.2*B*q   (> 0: lift leaves less sink)
    //   That gets *0.98 below and becomes the next tick's vy0. The next tick's
    //   convert depends on its vy0 at rate -0.1*q', so d²convert'/(dq' dvy0') = -0.1.
    //   => U_t,t+1 = -0.1*k1*0.98*(B - 0.1*A - 0.2*B*q)   (lift now steals next tick's sink)
    let vy_fall = vy_grav + convert;

    // the 0.9 is from turning
    let vz_turn = vz0 + convert * 0.9;

    // A unit of convert ends up as 0.98 in y and 0.9*0.99 = 0.891 in z, so it is worth
    //   k1 = 0.98*a_y + 0.891*a_z
    // where a_y, a_z are the marginal values of the returned y, z (the costates).
    // The objective only enters through k1, and k1 > 0 in the dive.
    Vec3 {
        x: 0.0,
        y: vy_fall * 0.98,
        z: vz_turn * 0.99,
    }
}

// Pair test: +d on q at tick t and -d at tick t+1 changes U by
//   d² * (U_tt - U_t,t+1) = d² * k1 * (-0.00612 - 0.0098*A - 0.00118*q)
// This is positive (alternation pays) iff
//   A < -0.6245 - 0.12*q   <=>   vy0 < -0.544 - 0.12*q   (about -0.60 at 45°)
