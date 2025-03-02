#[cfg(test)]
mod tests {
    use std::time::SystemTime;
    extern crate test;

    use num_bigint::{BigInt, BigUint, RandBigInt, ToBigInt};
    use num_integer::Integer;
    use num_traits::{FromPrimitive, ToPrimitive};
    use rand::{rngs::StdRng, Rng, SeedableRng};
    use test::Bencher;

    use crate::{
        algorithms::{generate_private_key, hash},
        PrivateKey, PublicKey,
    };

    #[test]
    fn test_from_into() {
        let private_key = PrivateKey {
            pubkey_components: PublicKey {
                n: BigUint::from_u64(100).unwrap(),
            },
            primes: vec![],
            hmac_secret: [0u8; 8],
        };
        let public_key: PublicKey = private_key.to_public_key();

        assert_eq!(public_key.n.to_u64(), Some(100));
    }

    fn test_key_basics(private_key: &PrivateKey) {
        private_key.validate().expect("invalid private key");

        let _pub_key: PublicKey = private_key.to_public_key();
        let _m = vec![42];
    }

    #[test]
    fn test_signing() {
        // Alice computes her private key
        let p = BigUint::from_u8(11u8).unwrap();
        let q = BigUint::from_u8(7u8).unwrap();
        let n = p.clone() * q.clone();
        let hmac_secret = [0u8; 8];
        let private_key = PrivateKey {
            pubkey_components: PublicKey { n },
            primes: vec![p, q],
            hmac_secret,
        };
        assert!(private_key.validate().is_ok());
        // And a public key for Bob
        let public_key: PublicKey = private_key.to_public_key();
        // Sign the message
        let message = String::from("fast verification scheme");
        let signature = private_key.sign(message.as_bytes());
        assert!(public_key.verify(message.as_bytes(), signature.unwrap()));
    }

    macro_rules! key_generation {
        ($name:ident,  $size:expr) => {
            #[test]
            fn $name() {
                let seed = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap();
                let mut rng = StdRng::seed_from_u64(seed.as_secs());

                for _ in 0..10 {
                    let private_key = generate_private_key(&mut rng, $size).unwrap();
                    assert_eq!(private_key.n.bits(), $size);

                    test_key_basics(&private_key);
                }
            }
        };
    }

    key_generation!(key_generation_128, 128);
    // key_generation!(key_generation_1024, 1024);

    // key_generation!(key_generation_multi_3_256, 256);

    key_generation!(key_generation_multi_4_64, 64);

    key_generation!(key_generation_multi_5_64, 64);
    // key_generation!(key_generation_multi_8_576, 576);
    // key_generation!(key_generation_multi_16_1024, 1024);

    #[test]
    fn test_impossible_keys() {
        // make sure not infinite loops are hit here.
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());
        for i in 0..12 {
            assert!(generate_private_key(&mut rng, i).is_err());
        }
        assert!(generate_private_key(&mut rng, 13).is_ok());
    }

    #[test]
    #[cfg(feature = "serde1")]
    fn test_serde() {
        use rand::SeedableRng;
        use rand_xorshift::XorShiftRng;
        use serde_test::{assert_tokens, Token};

        let mut rng = XorShiftRng::from_seed([1; 16]);
        let priv_key = PrivateKey::new(&mut rng, 64).expect("failed to generate key");

        let priv_tokens = [
            Token::Struct {
                name: "RsaPrivateKey",
                len: 3,
            },
            Token::Str("pubkey_components"),
            Token::Struct {
                name: "RsaPublicKey",
                len: 2,
            },
            Token::Str("n"),
            Token::Seq { len: Some(2) },
            Token::U32(1296829443),
            Token::U32(2444363981),
            Token::SeqEnd,
            Token::Str("e"),
            Token::Seq { len: Some(1) },
            Token::U32(65537),
            Token::SeqEnd,
            Token::StructEnd,
            Token::Str("d"),
            Token::Seq { len: Some(2) },
            Token::U32(298985985),
            Token::U32(2349628418),
            Token::SeqEnd,
            Token::Str("primes"),
            Token::Seq { len: Some(2) },
            Token::Seq { len: Some(1) },
            Token::U32(3238068481),
            Token::SeqEnd,
            Token::Seq { len: Some(1) },
            Token::U32(3242199299),
            Token::SeqEnd,
            Token::SeqEnd,
            Token::StructEnd,
        ];
        assert_tokens(&priv_key, &priv_tokens);

        let priv_tokens = [
            Token::Struct {
                name: "RsaPublicKey",
                len: 2,
            },
            Token::Str("n"),
            Token::Seq { len: Some(2) },
            Token::U32(1296829443),
            Token::U32(2444363981),
            Token::SeqEnd,
            Token::Str("e"),
            Token::Seq { len: Some(1) },
            Token::U32(65537),
            Token::SeqEnd,
            Token::StructEnd,
        ];
        assert_tokens(&PublicKey::from(priv_key), &priv_tokens);
    }

    #[bench]
    fn biguint_to_bigint(b: &mut Bencher) {
        const SAMPLES: usize = 1000;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let bigints = (0..SAMPLES)
            .map(|_| rng.gen_biguint(1024))
            .collect::<Vec<BigUint>>();

        let mut i = 0;
        b.iter(|| {
            bigints[i % SAMPLES].to_bigint().unwrap();
            i += 1;
        });
    }

    #[bench]
    fn biguint_mod_floor(b: &mut Bencher) {
        const SAMPLES: usize = 1000;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let bigints = (0..SAMPLES)
            .map(|_| rng.gen_biguint(1024))
            .collect::<Vec<BigUint>>();
        let n = rng.gen_biguint(1024);
        let mut i = 0;
        b.iter(|| {
            bigints[i % SAMPLES].mod_floor(&n);
            i += 1;
        });
    }

    #[bench]
    fn bigint_mod_floor(b: &mut Bencher) {
        const SAMPLES: usize = 1000;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let bigints = (0..SAMPLES)
            .map(|_| rng.gen_bigint(2048))
            .collect::<Vec<BigInt>>();
        let n = rng.gen_bigint(1024);
        let mut i = 0;
        b.iter(|| {
            bigints[i % SAMPLES].mod_floor(&n);
            i += 1;
        });
    }

    #[bench]
    fn bigint_to_biguint(b: &mut Bencher) {
        const SAMPLES: usize = 1000;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let bigints = (0..SAMPLES)
            .map(|_| rng.gen_bigint(1024))
            .collect::<Vec<BigInt>>();

        let mut i = 0;
        b.iter(|| {
            bigints[i % SAMPLES].to_biguint();
            i += 1;
        });
    }

    #[bench]
    fn biguint_mul(b: &mut Bencher) {
        const SAMPLES: usize = 1000;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let bigints = (0..SAMPLES)
            .map(|_| rng.gen_bigint(1024))
            .collect::<Vec<BigInt>>();

        let mut i = 0;
        b.iter(|| {
            let _ = &bigints[i % SAMPLES] * &bigints[(i + 1) % SAMPLES];
            i += 1;
        });
    }

    #[bench]
    fn biguint_triple_mul(b: &mut Bencher) {
        const SAMPLES: usize = 1000;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let bigints = (0..SAMPLES)
            .map(|_| rng.gen_bigint(1024))
            .collect::<Vec<BigInt>>();

        let n = rng.gen_bigint(1024);

        let mut i = 0;
        b.iter(|| {
            (&bigints[i % SAMPLES] * &bigints[(i + 1) % SAMPLES] * &bigints[(i + 1) % SAMPLES])
                .mod_floor(&n);
            i += 1;
        });
    }

    #[bench]
    fn test_hash(b: &mut Bencher) {
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let msg: &[u8] = &rng.gen::<[u8; 32]>();

        let mut i = 0;
        b.iter(|| {
            hash(msg);
            i += 1;
        });
    }

    #[bench]
    fn test_sqrt(b: &mut Bencher) {
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let n = rng.gen_biguint(1024);

        let mut i = 0;
        b.iter(|| {
            n.sqrt();
            i += 1;
        });
    }
    
    #[bench]
    fn test_deser(b: &mut Bencher) {
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let mut rng = StdRng::seed_from_u64(seed.as_secs());

        let msg: &[u8] = &rng.gen::<[u8; 32]>();

        let mut i = 0;
        b.iter(|| {
            BigUint::from_bytes_be(msg);
            i += 1;
        });
    }
}
