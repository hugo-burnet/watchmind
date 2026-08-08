use watchmind_infrastructure::AniListNormalizer;

#[test]
fn normalizes_anilist_fixture_without_network() {
    let payload = include_str!("../../../fixtures/anilist/search-anime.json");
    let works = AniListNormalizer::normalize(payload).expect("fixture should normalize");

    assert_eq!(works.len(), 1);
    let work = &works[0];
    assert_eq!(work.id().get(), 1535);
    assert_eq!(work.title(), "Death Note");
    assert!((work.global_score().expect("score").get() - 8.4).abs() < f64::EPSILON);
    assert_eq!(work.runtime_minutes().expect("runtime").get(), 851);
    assert_eq!(work.release_year().expect("year").get(), 2006);
    assert_eq!(work.studios(), &["Madhouse"]);
    assert_eq!(work.tags().len(), 2, "spoiler tags must be excluded");
}

#[test]
fn exposes_graphql_errors() {
    let error = AniListNormalizer::normalize(r#"{"errors":[{"message":"rate limited"}]}"#)
        .expect_err("GraphQL errors must not be ignored");
    assert!(error.to_string().contains("rate limited"));
}
