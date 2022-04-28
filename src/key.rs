use alloc::vec::Vec;
use core::ops::Deref;

use num_bigint::{BigInt, BigUint, ToBigInt};
use num_integer::Integer;
use num_traits::{FromPrimitive, One};
#[cfg(feature = "serde")]
use serde_crate::{Deserialize, Serialize};

use crate::{
    algorithms::calculate_tweak_factors,
    errors::{Error, Result},
};

/// Default exponent for RSA keys.
const EXP: u8 = 2;

/// Represents the public part of an RSA key.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub struct PublicKey {
    pub n: BigUint,
}

/// Represents a whole RSA key, public and private parts.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub struct PrivateKey {
    /// Public components of the private key.
    pub pubkey_components: PublicKey,
    /// Prime factors of N, contains >= 2 elements.
    pub(crate) primes: Vec<BigUint>,
}

impl Deref for PrivateKey {
    type Target = PublicKey;
    fn deref(&self) -> &PublicKey {
        &self.pubkey_components
    }
}

impl PublicKey {
    /// Create a new key from its components.
    pub fn new(n: BigUint) -> Result<Self> {
        Ok(PublicKey { n })
    }
}

impl PrivateKey {
    /// Constructs an RSA key pair from the individual components.
    pub fn from_components(n: BigUint, primes: Vec<BigUint>) -> PrivateKey {
        PrivateKey {
            pubkey_components: PublicKey { n },
            primes,
        }
    }

    /// Get the public key from the private key, cloning `n` and `e`.
    ///
    /// Generally this is not needed since `RsaPrivateKey` implements the `PublicKey` trait,
    /// but it can occationally be useful to discard the private information entirely.
    pub fn to_public_key(&self) -> PublicKey {
        // Safe to unwrap since n and e are already verified.
        self.pubkey_components.clone()
    }

    /// Returns the prime factors.
    pub fn primes(&self) -> &[BigUint] {
        &self.primes
    }

    /// Performs basic sanity checks on the key.
    /// Returns `Ok(())` if everything is good, otherwise an approriate error.
    pub fn validate(&self) -> Result<()> {
        // Check that Πprimes == n.
        let mut m = BigUint::one();
        for prime in &self.primes {
            if *prime < BigUint::one() {
                return Err(Error::InvalidPrime);
            }
            m *= prime;
        }
        if m != self.n {
            return Err(Error::InvalidModulus);
        }

        Ok(())
    }

    /// Compute the sqrt of `c` mod n, where n is composite
    /// First, the quadratic residuosity test is performed by computing
    /// Legendre Symbol L. If L == 1, proceed to computing individual sqrt mod p and mod q.
    /// Finally, combine the two using Chinese Remainder Theorem.
    pub(crate) fn sqrt_mod_pq(&self, c: &BigUint) -> (BigUint, i8, u8) {
        // For the case of only two primes
        let p = self.primes[0].clone();
        let q = self.primes[1].clone();

        // first, checking that Legendre == 1
        let legendre_p: BigUint = c.modpow(
            &((&p - BigUint::one()) / BigUint::from_u8(2u8).unwrap()),
            &p,
        );
        let legendre_q: BigUint = c.modpow(
            &((&q - BigUint::one()) / BigUint::from_u8(2u8).unwrap()),
            &q,
        );
        let a = legendre_p == BigUint::one();
        let b = legendre_q == BigUint::one();

        let (e, f) = calculate_tweak_factors(a, b);

        // Calculate e*f*H(m), which should be a square mod n
        let h: BigUint = (c.to_bigint().unwrap() * e * f)
            .mod_floor(&self.n.to_bigint().unwrap())
            .to_biguint()
            .unwrap();

        (self.combine_sqrt(&h, p, q), e, f)
    }

    fn combine_sqrt(&self, c: &BigUint, p: BigUint, q: BigUint) -> BigUint {
        // Now use Chinese Remainder Theorem to compute x mod n
        // Generalised CRT is stated as:
        // x == a_0 mod (n_0)
        // x == a_1 mod (n_1)
        // ...
        // x == a_(k-1) mod (n_(k-1))
        // And the solution is given by:
        // x = x_0 * N_0 * a_0 + ... + x_(k-1) * N_(k-1) * a_(k-1)
        // where:
        // N_i = n / n_i
        // N_i * x_i == 1

        // For the specific case:
        // x == a_0 mod p
        // x == a_1 mod q
        //
        // a_0 == sqrt(c) mod p
        // a_1 == sqrt(c) mod q
        //
        // n = p * q
        // n_0 = p
        // n_1 = q
        // N_0 = n / n_0 = n / p = q
        // N_1 = p

        // Pre-compute the exponents
        // + Sanity check: since prime == 3 mod 4, the remainder should always be 0
        let (exponent_p, remainder) = (&p + BigUint::one()).div_mod_floor(&BigUint::from(4u8));
        assert_eq!(remainder, BigUint::from(0u8));
        let (exponent_q, remainder) = (&q + BigUint::one()).div_mod_floor(&BigUint::from(4u8));
        assert_eq!(remainder, BigUint::from(0u8));

        // Compute the intermediate sqrt values modulo p and modulo q
        let a_0: BigInt = BigInt::from(c.modpow(&exponent_p, &p));

        let a_1: BigInt = BigInt::from(c.modpow(&exponent_q, &q));

        // from Extended Euclidian Algorithm, we get Bezout's coefficients x & y s.t.:
        // 1 == gcd(p,q) == p*x + q*y
        let ee = p.to_bigint().unwrap().extended_gcd(&q.to_bigint().unwrap());

        let x = &ee.x;
        let y = &ee.y;
        // Some sanity checks
        assert!(ee.gcd.is_one());
        assert_eq!(
            BigInt::one(),
            x * p.to_bigint().unwrap() + &(y * &q.to_bigint().unwrap())
        );
        // Check that p * x == 1 mod q
        // i.e. that N_1 * x == 1 (mod n_1)
        assert_eq!(
            (x * &p.to_bigint().unwrap()).mod_floor(&q.to_bigint().unwrap()),
            BigInt::one()
        );
        // By symmetry: q * y == 1 mod p
        assert_eq!(
            (y * &q.to_bigint().unwrap()).mod_floor(&p.to_bigint().unwrap()),
            BigInt::one()
        );
        // Compute the final combined x
        let x: BigInt = (y * q.to_bigint().unwrap() * &a_0 + x * p.to_bigint().unwrap() * &a_1)
            % self.n.to_bigint().unwrap();

        let x = x
            .mod_floor(&self.n.to_bigint().unwrap())
            .to_biguint()
            .unwrap();
        // Final correctness check: x^2 == c mod n
        assert_eq!(c, &(x.modpow(&BigUint::from(EXP), &self.n)));

        x
    }
}
