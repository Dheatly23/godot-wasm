use std::ops;

pub trait Derivable {
    type Deriv;

    fn deriv(&self, t: f64) -> Self::Deriv;
}

pub fn runge_kutta<T, D>(t: f64, dt: f64, y: T) -> T
where
    T: Derivable<Deriv = D> + ops::Add<D, Output = T>,
    for<'a> &'a T: ops::Add<D, Output = T>,
    D: ops::Mul<f64, Output = D>,
    for<'a> &'a D: ops::Mul<f64, Output = D>,
{
    const C1: f64 = 1.0 / 6.0;
    const C2: f64 = 1.0 / 3.0;
    const C3: f64 = 1.0 / 3.0;
    const C4: f64 = 1.0 / 6.0;

    let th = t + dt * 0.5;

    let mut d = y.deriv(t);
    let k1 = &d * dt;
    let mut ret = &y + &k1 * 0.5;

    d = ret.deriv(th);
    let k2 = &d * dt;
    ret = &y + &k2 * 0.5;

    d = ret.deriv(th);
    let k3 = &d * dt;
    ret = &y + &k3 * 1.0;

    d = ret.deriv(t + dt);
    let k4 = d * dt;
    ret = y + k1 * C1 + k2 * C2 + k3 * C3 + k4 * C4;

    return ret;
}
