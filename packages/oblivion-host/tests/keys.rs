use oblivion_host::keys::{Archive, Command, LEGACY};
use zeroize::Zeroizing;

const FIRST: &str = "synthetic-first-api-key";
const SECOND: &str = "synthetic-second-api-key";

#[test]
fn named_keys_roundtrip_without_disclosing_secrets_in_status() {
    let mut archive = Archive::empty();
    archive
        .change(Command::Add {
            name: "Personale".into(),
            key: Zeroizing::new(FIRST.into()),
        })
        .unwrap();
    archive
        .change(Command::Add {
            name: "Lavoro".into(),
            key: Zeroizing::new(SECOND.into()),
        })
        .unwrap();
    let archive = Archive::decode(&archive.encode().unwrap()).unwrap();
    let status = archive.status(None);
    assert_eq!(status.entries.len(), 2);
    assert_ne!(status.entries[0].id, status.entries[1].id);
    assert_eq!(status.selected.as_ref(), Some(&status.entries[0].id));
    assert_eq!(archive.key(&status.entries[1].id), Some(SECOND));
    let public = serde_json::to_string(&status).unwrap();
    assert!(public.contains("Personale") && public.contains("Lavoro"));
    assert!(!public.contains(FIRST) && !public.contains(SECOND));
}

#[test]
fn selection_overrides_default_without_rotating_after_removal() {
    let mut archive = Archive::legacy(Zeroizing::new(FIRST.into())).unwrap();
    archive
        .change(Command::Add {
            name: "Seconda".into(),
            key: Zeroizing::new(SECOND.into()),
        })
        .unwrap();
    let next = archive.status(None).entries[1].id.clone();
    archive
        .change(Command::Default { id: next.clone() })
        .unwrap();
    assert_eq!(archive.status(None).selected, Some(next.clone()));
    assert_eq!(
        archive
            .status(Some(Some(LEGACY.into())))
            .selected
            .as_deref(),
        Some(LEGACY)
    );
    assert!(archive.status(Some(None)).selected.is_none());
    archive
        .change(Command::Remove { id: LEGACY.into() })
        .unwrap();
    assert!(archive.status(Some(Some(LEGACY.into()))).selected.is_none());
    archive.change(Command::Remove { id: next }).unwrap();
    assert!(archive.status(None).default.is_none());
    assert!(archive.status(None).selected.is_none());
}

#[test]
fn replacement_and_rename_preserve_identity_and_invalid_changes_preserve_data() {
    let mut archive = Archive::legacy(Zeroizing::new(FIRST.into())).unwrap();
    archive
        .change(Command::Rename {
            id: LEGACY.into(),
            name: "Privata".into(),
        })
        .unwrap();
    archive
        .change(Command::Replace {
            id: LEGACY.into(),
            key: Zeroizing::new(SECOND.into()),
        })
        .unwrap();
    assert_eq!(archive.status(None).entries[0].name, "Privata");
    assert_eq!(archive.key(LEGACY), Some(SECOND));
    let before = archive.encode().unwrap();
    for name in ["", " spazio", "fine ", "escape\x1b[2J", "due\nrighe"] {
        assert!(
            archive
                .change(Command::Rename {
                    id: LEGACY.into(),
                    name: name.into()
                })
                .is_err()
        );
    }
    assert!(
        archive
            .change(Command::Replace {
                id: LEGACY.into(),
                key: Zeroizing::new("invalid key".into())
            })
            .is_err()
    );
    assert!(
        archive
            .change(Command::Default {
                id: "missing".into()
            })
            .is_err()
    );
    assert_eq!(*before, *archive.encode().unwrap());
}

#[test]
fn malformed_archives_never_fall_back_to_legacy_or_another_key() {
    let archive = Archive::legacy(Zeroizing::new(FIRST.into())).unwrap();
    let original: serde_json::Value = serde_json::from_slice(&archive.encode().unwrap()).unwrap();
    for field in ["version", "default", "entries", "extra"] {
        let mut invalid = original.clone();
        invalid[field] = serde_json::json!("invalid");
        assert!(Archive::decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let mut duplicate = original.clone();
    duplicate["entries"]
        .as_array_mut()
        .unwrap()
        .push(original["entries"][0].clone());
    assert!(Archive::decode(&serde_json::to_vec(&duplicate).unwrap()).is_err());
    assert!(Archive::decode(b"{}").is_err());
    assert!(
        serde_json::from_str::<Command>(r#"{"action":"select","id":"a","key":"not-allowed"}"#)
            .is_err()
    );
}
