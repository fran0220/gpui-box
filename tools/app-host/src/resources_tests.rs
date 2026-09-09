use super::*;

fn registration(key: &str, mime: &str, bytes: &[u8]) -> Registration {
    Registration {
        key: key.into(),
        mime: mime.into(),
        data: STANDARD.encode(bytes),
    }
}
fn pixels() -> Vec<u8> {
    // Asymmetric red then blue pixels, not a channel-symmetric white fixture.
    [
        2u32.to_le_bytes().as_slice(),
        1u32.to_le_bytes().as_slice(),
        &[255, 0, 0, 255, 0, 0, 255, 255],
    ]
    .concat()
}
fn store(owner: EffectOwner) -> ResourceStore {
    ResourceStore(Rc::new(RefCell::new(State {
        owners: HashMap::from([(owner, HashMap::new())]),
        mounts: HashMap::from([(
            owner,
            Mount {
                principal: owner,
                identity: Rc::new(()),
            },
        )]),
    })))
}

#[test]
fn rejects_paths_mime_compression_corruption_and_allocation_before_consuming_quota() {
    let owner = EffectOwner::new();
    let mut store = store(owner);
    for key in [
        "",
        "..",
        "a/b",
        "a\\b",
        "file:///etc/passwd",
        "https://x",
        "é",
        "%2e%2e",
    ] {
        assert!(
            store
                .register(owner, registration(key, RGBA, &pixels()))
                .is_err(),
            "{key}"
        );
    }
    for mime in [
        "image/png",
        "image/jpeg",
        "image/gif",
        "image/svg+xml",
        "text/html",
    ] {
        assert!(
            store
                .register(owner, registration("x", mime, &[0; 64]))
                .is_err()
        );
    }
    for bytes in [
        vec![],
        vec![0; 7],
        vec![255; 8],
        [1024u32.to_le_bytes(), 1024u32.to_le_bytes()].concat(),
        [pixels(), vec![0]].concat(),
    ] {
        assert!(
            store
                .register(owner, registration("x", RGBA, &bytes))
                .is_err()
        );
    }
    for data in ["!!!!", "Zg", "Zh==", "Zg==\n", ""] {
        assert!(
            store
                .register(
                    owner,
                    Registration {
                        key: "x".into(),
                        mime: BYTES.into(),
                        data: data.into()
                    }
                )
                .is_err()
        );
    }
    assert!(
        store
            .register(owner, registration("x", BYTES, &vec![0; MAX_BYTES + 1]))
            .is_err()
    );
    assert!(store.0.borrow().owners[&owner].is_empty());
    let reference = store
        .register(owner, registration("x", RGBA, &pixels()))
        .expect("valid raw image");
    let state = store.0.borrow();
    let Asset::Image(image) = &state.owners[&owner][&reference.key].asset else {
        panic!("not image")
    };
    assert_eq!(
        image.as_bytes(0).expect("first image frame"),
        &[0, 0, 255, 255, 255, 0, 0, 255]
    );
    assert_eq!(image.size(0), size(2.into(), 1.into()));
    assert!(serde_json::from_str::<ResourceRef>(r#"{"key":"x","owner":5}"#).is_err());
    assert!(
        serde_json::from_str::<Registration>(
            r#"{"key":"x","mime":"image/png","data":"","url":"https://x"}"#
        )
        .is_err()
    );
}

#[test]
fn quotas_are_per_owner_and_duplicates_cannot_replace_bytes() {
    let owner = EffectOwner::new();
    let mut store = store(owner);
    let foreign = EffectOwner::new();
    assert!(
        store
            .register(foreign, registration("x", BYTES, &[1]))
            .is_err()
    );
    for index in 0..MAX_OWNER_COUNT {
        store
            .register(owner, registration(&format!("r{index}"), BYTES, &[1]))
            .expect("within count quota");
    }
    assert!(
        store
            .register(owner, registration("overflow", BYTES, &[2]))
            .is_err()
    );
    assert!(
        store
            .register(owner, registration("r0", BYTES, &[2]))
            .is_err()
    );
    let Asset::Bytes(bytes) = &store.0.borrow().owners[&owner]["r0"].asset else {
        panic!("bytes")
    };
    assert_eq!(bytes, &[1]);
    let mut other = self::store(foreign);
    for index in 0..10 {
        other
            .register(
                foreign,
                registration(&format!("large{index}"), BYTES, &vec![0; MAX_BYTES]),
            )
            .expect("within byte quota");
    }
    assert!(
        other
            .register(
                foreign,
                registration("overflow", BYTES, &vec![0; MAX_BYTES])
            )
            .is_err()
    );
    // 1 MiB minus 10 * 96 KiB leaves exactly 64 KiB.
    other
        .register(foreign, registration("tail", BYTES, &vec![0; 65536]))
        .expect("exact aggregate boundary");
    assert!(
        other
            .register(foreign, registration("last", BYTES, &[1]))
            .is_err()
    );
}

#[cfg(feature = "capture")]
#[gpui::test]
fn leases_revalidate_owner_generation_revocation_and_host_drop(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let owner = EffectOwner::new();
        let foreign = EffectOwner::new();
        let mut store = Resources::install(cx);
        store.reconcile(&HashMap::from([(owner, owner), (foreign, foreign)]), cx);
        let reference = store
            .register(owner, registration("data", BYTES, &[7, 9]))
            .expect("authorized bytes");
        assert!(Resources::bytes(&reference, cx).is_err());
        let bytes = cx.with_effect_owner(Some(owner), |cx| {
            Resources::bytes(&reference, cx).expect("authorized lease")
        });
        cx.with_effect_owner(Some(foreign), |cx| {
            assert!(Resources::bytes(&reference, cx).is_err());
            assert!(
                bytes
                    .with_bytes::<()>(cx, |_| panic!("foreign access"))
                    .is_err()
            );
        });
        cx.with_effect_owner(Some(owner), |cx| {
            assert_eq!(
                bytes
                    .with_bytes(cx, |data| Ok(data.to_vec()))
                    .expect("authorized use"),
                [7, 9]
            );
            assert!(Resources::image(&reference, cx).is_err());
        });
        store.revoke(owner, cx);
        store.reconcile(&HashMap::from([(owner, owner)]), cx);
        store
            .register(owner, registration("data", BYTES, &[11]))
            .expect("new registration after reauthorization");
        cx.with_effect_owner(Some(owner), |cx| {
            assert!(bytes.with_bytes(cx, |_| Ok(())).is_err())
        });
        let fresh = cx.with_effect_owner(Some(owner), |cx| {
            Resources::bytes(&reference, cx).expect("fresh lease")
        });
        store.reconcile(&HashMap::from([(foreign, foreign)]), cx);
        cx.with_effect_owner(Some(owner), |cx| {
            assert!(fresh.with_bytes(cx, |_| Ok(())).is_err())
        });
        store
            .register(foreign, registration("data", BYTES, &[13]))
            .expect("new owner data");
        let final_lease = cx.with_effect_owner(Some(foreign), |cx| {
            Resources::bytes(&reference, cx).expect("lease before drop")
        });
        drop(store);
        cx.with_effect_owner(Some(foreign), |cx| {
            assert!(final_lease.with_bytes(cx, |_| Ok(())).is_err())
        });
    });
}

#[cfg(feature = "capture")]
#[gpui::test]
fn mount_aliases_share_generation_quota_but_not_lifetime(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let principal = EffectOwner::new();
        let a = EffectOwner::new();
        let b = EffectOwner::new();
        let mut store = Resources::install(cx);
        store.reconcile(&HashMap::from([(a, principal), (b, principal)]), cx);
        let reference = store
            .register(principal, registration("shared", BYTES, &[17, 23]))
            .expect("principal registration");
        assert!(
            store
                .register(a, registration("not-principal", BYTES, &[1]))
                .is_err()
        );
        let lease_a = cx.with_effect_owner(Some(a), |cx| {
            Resources::bytes(&reference, cx).expect("mount a")
        });
        let lease_b = cx.with_effect_owner(Some(b), |cx| {
            Resources::bytes(&reference, cx).expect("mount b")
        });
        assert_eq!(store.0.borrow().owners.len(), 1);
        assert_eq!(store.0.borrow().owners[&principal].len(), 1);
        for index in 1..MAX_OWNER_COUNT {
            store
                .register(
                    principal,
                    registration(&format!("data{index}"), BYTES, &[1]),
                )
                .expect("shared quota");
        }
        assert!(
            store
                .register(principal, registration("over", BYTES, &[1]))
                .is_err()
        );
        cx.with_effect_owner(Some(b), |cx| {
            assert!(lease_a.with_bytes(cx, |_| Ok(())).is_err());
            assert_eq!(
                lease_b
                    .with_bytes(cx, |bytes| Ok(bytes.to_vec()))
                    .expect("peer access"),
                [17, 23]
            );
        });
        store.reconcile(&HashMap::from([(b, principal)]), cx);
        cx.with_effect_owner(Some(a), |cx| {
            assert!(lease_a.with_bytes(cx, |_| Ok(())).is_err())
        });
        cx.with_effect_owner(Some(b), |cx| {
            assert!(lease_b.with_bytes(cx, |_| Ok(())).is_ok())
        });
        // Even a reused mount token cannot resurrect its previous lazy lease.
        store.reconcile(&HashMap::from([(a, principal), (b, principal)]), cx);
        cx.with_effect_owner(Some(a), |cx| {
            assert!(lease_a.with_bytes(cx, |_| Ok(())).is_err());
            assert!(Resources::bytes(&reference, cx).is_ok());
        });
        store.revoke(principal, cx);
        assert!(store.0.borrow().owners.is_empty());
        assert!(store.0.borrow().mounts.is_empty());
        cx.with_effect_owner(Some(b), |cx| {
            assert!(lease_b.with_bytes(cx, |_| Ok(())).is_err())
        });

        store.reconcile(&HashMap::from([(b, principal)]), cx);
        store
            .register(principal, registration("shared", BYTES, &[29]))
            .expect("reauthorized generation");
        let replacement_lease = cx.with_effect_owner(Some(b), |cx| {
            Resources::bytes(&reference, cx).expect("replacement lease")
        });
        let _replacement_host = Resources::install(cx);
        cx.with_effect_owner(Some(b), |cx| {
            assert!(replacement_lease.with_bytes(cx, |_| Ok(())).is_err())
        });
        assert!(
            store.0.borrow().owners.is_empty(),
            "host replacement revokes still-retained old host"
        );
    });
}
