mod deriv_trait;

use std::ops;
use std::ptr::addr_of;

const G: f64 = 9.8;

#[derive(Clone, Copy)]
struct PendulumConfig {
    m1: f64,
    m2: f64,
    l1: f64,
    l2: f64,
    delta: f64,
}

#[derive(Clone, Copy)]
struct PendulumState {
    config: PendulumConfig,

    theta1: f64,
    w1: f64,
    theta2: f64,
    w2: f64,
}

static mut STATE: PendulumState = PendulumState {
    config: PendulumConfig {
        m1: 0.0,
        m2: 0.0,
        l1: 0.0,
        l2: 0.0,
        delta: 0.0,
    },

    theta1: 0.0,
    w1: 0.0,
    theta2: 0.0,
    w2: 0.0,
};
static mut T: f64 = 0.0;

#[no_mangle]
pub extern "C" fn setup(
    m1: f64,
    m2: f64,
    l1: f64,
    l2: f64,
    delta: f64,
    theta1: f64,
    w1: f64,
    theta2: f64,
    w2: f64,
) {
    unsafe {
        STATE = PendulumState {
            config: PendulumConfig {
                m1,
                m2,
                l1,
                l2,
                delta,
            },
            theta1,
            w1,
            theta2,
            w2,
        };
    }
}

static mut OUTPUT: [f64; 4] = [0.0; 4];

#[no_mangle]
pub extern "C" fn process(mut delta: f64) -> *const f64 {
    let (mut s, mut t) = unsafe { (STATE, T) };
    let dt = s.config.delta;

    while delta >= dt {
        s = deriv_trait::runge_kutta(t, dt, s);
        t += dt;
        delta -= dt;
    }

    unsafe {
        STATE = s;
        T = t;
        OUTPUT = [s.theta1, s.w1, s.theta2, s.w2];
        addr_of!(OUTPUT[0])
    }
}

#[derive(Default, Clone, Copy)]
struct PendulumDeriv {
    dt1: f64,
    dw1: f64,
    dt2: f64,
    dw2: f64,
}

impl deriv_trait::Derivable for PendulumState {
    type Deriv = PendulumDeriv;

    fn deriv(&self, _t: f64) -> PendulumDeriv {
        // Translated from: http://www.physics.usyd.edu.au/~wheat/dpend_html/solve_dpend.c

        let delta = self.theta2 - self.theta1;
        let (sd, cd) = delta.sin_cos();
        let den1 = (self.config.m1 + self.config.m2) * self.config.l1
            - self.config.l2 * self.config.l1 * cd * cd;
        let den2 = (self.config.l2 / self.config.l1) * den1;

        PendulumDeriv {
            dt1: self.w1,
            dw1: (self.config.m2 * self.config.l1 * self.w1 * self.w1 * sd * cd
                + self.config.m2 * G * self.theta2.sin() * cd
                + self.config.m2 * self.config.l2 * self.w2 * self.w2 * sd
                - (self.config.m1 + self.config.m2) * G * self.theta1.sin())
                / den1,
            dt2: self.w2,
            dw2: (-self.config.m2 * self.config.l2 * self.w2 * self.w2 * sd * cd
                + (self.config.m1 + self.config.m2) * G * self.theta1.sin() * cd
                - (self.config.m1 + self.config.m2) * self.config.l1 * self.w1 * self.w1 * sd
                - (self.config.m1 + self.config.m2) * G * self.theta2.sin())
                / den2,
        }
    }
}

impl<'a> ops::Mul<f64> for &'a PendulumDeriv {
    type Output = PendulumDeriv;

    fn mul(self, k: f64) -> PendulumDeriv {
        PendulumDeriv {
            dt1: self.dt1 * k,
            dw1: self.dw1 * k,
            dt2: self.dt2 * k,
            dw2: self.dw2 * k,
        }
    }
}

impl ops::Mul<f64> for PendulumDeriv {
    type Output = Self;

    fn mul(mut self, k: f64) -> Self {
        self.dt1 *= k;
        self.dw1 *= k;
        self.dt2 *= k;
        self.dw2 *= k;
        self
    }
}

fn wrap_theta(v: f64) -> f64 {
    use std::f64::consts::PI;
    let d = (v.div_euclid(PI) * 0.5).trunc();
    v - d * (PI * 2.0)
}

impl<'a> ops::Add<PendulumDeriv> for &'a PendulumState {
    type Output = PendulumState;

    fn add(self, d: PendulumDeriv) -> PendulumState {
        PendulumState {
            config: self.config,

            theta1: wrap_theta(self.theta1 + d.dt1),
            w1: self.w1 + d.dw1,
            theta2: wrap_theta(self.theta2 + d.dt2),
            w2: self.w2 + d.dw2,
        }
    }
}

impl ops::Add<PendulumDeriv> for PendulumState {
    type Output = Self;

    fn add(mut self, d: PendulumDeriv) -> Self {
        self.theta1 = wrap_theta(self.theta1 + d.dt1);
        self.w1 += d.dw1;
        self.theta2 = wrap_theta(self.theta2 + d.dt2);
        self.w2 += d.dw2;
        self
    }
}
