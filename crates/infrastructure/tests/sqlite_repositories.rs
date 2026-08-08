use serde_json::json;
use tempfile::tempdir;
use watchmind_infrastructure::{Database, LibraryEntry};
use watchmind_recommendation::{
    AspectCredit, DropProgress, NormalizedWork, PersonalAxis, Rating, RatingRecord, TagWeight,
    WatchEvent, Weight, WorkId,
};

fn sample_work() -> NormalizedWork {
    NormalizedWork::new(
        WorkId::new(1535).unwrap(),
        "Death Note",
        Some(Rating::new(8.4).unwrap()),
        vec![TagWeight::new("Crime", Weight::new(0.9).unwrap()).unwrap()],
    )
    .unwrap()
}

#[tokio::test]
async fn migrates_and_round_trips_every_repository() {
    let db = Database::in_memory().await.unwrap();
    let work = sample_work();
    db.works().upsert(&work).await.unwrap();

    let rating = RatingRecord::new(
        work.id(),
        Rating::new(9.0).unwrap(),
        vec![AspectCredit::new(PersonalAxis::Story, Weight::new(0.8).unwrap()).unwrap()],
    )
    .unwrap();
    let work_id = rating.work_id();
    db.ratings().upsert(&rating).await.unwrap();
    db.events()
        .append(&WatchEvent::dropped(
            work.id(),
            DropProgress::new(3, 37).unwrap(),
        ))
        .await
        .unwrap();
    db.preferences()
        .set("locale", &json!("fr-FR"))
        .await
        .unwrap();

    assert_eq!(db.works().get(work.id()).await.unwrap(), Some(work));
    assert_eq!(db.tags().for_work(rating.work_id()).await.unwrap().len(), 1);
    assert_eq!(
        db.ratings().get(rating.work_id()).await.unwrap(),
        Some(rating)
    );
    assert_eq!(db.events().for_work(work_id).await.unwrap().len(), 1);
    assert_eq!(db.aspects().for_work(work_id).await.unwrap().len(), 1);
    assert_eq!(
        db.preferences().get("locale").await.unwrap(),
        Some(json!("fr-FR"))
    );
}

#[tokio::test]
async fn exports_and_restores_a_database() {
    let directory = tempdir().unwrap();
    let backup = directory.path().join("watchmind-backup.json");
    let source = Database::in_memory().await.unwrap();
    let work = sample_work();
    source.works().upsert(&work).await.unwrap();
    source
        .events()
        .append(&WatchEvent::completed(work.id()))
        .await
        .unwrap();
    source
        .library()
        .upsert(&LibraryEntry {
            work_id: work.id(),
            comment: Some("Repère".to_owned()),
        })
        .await
        .unwrap();
    source
        .snapshots()
        .create(1_700_000_000, &json!({"history_size": 1}), &[])
        .await
        .unwrap();
    source.export(&backup).await.unwrap();

    let restored = Database::in_memory().await.unwrap();
    restored.restore(&backup).await.unwrap();
    assert_eq!(
        restored.works().get(work.id()).await.unwrap(),
        Some(work.clone())
    );
    assert_eq!(
        restored.events().for_work(work.id()).await.unwrap(),
        vec![WatchEvent::completed(work.id())]
    );
    assert_eq!(
        restored.library().get(work.id()).await.unwrap(),
        Some(LibraryEntry {
            work_id: work.id(),
            comment: Some("Repère".to_owned())
        })
    );
    assert_eq!(
        restored
            .snapshots()
            .latest_profile()
            .await
            .unwrap()
            .unwrap()
            .profile,
        json!({"history_size": 1})
    );
}
