//! `fixtures/gamma_events_keyset.json` is a verbatim `GET /events/keyset?id=34053&id=578465&id=28799&limit=3`
//! response captured on 2026-11-15. The other `gamma_*` fixtures are live responses captured on
//! 2026-11-16, trimmed to fewer rows (values unedited).

use roshar_polymarket::types::{
    EventsPage, GammaError, GammaEventsPagination, GammaMarketDescription, GammaMarketsPage,
    GammaRelatedTag, GammaSearch, GammaSeriesSummary, GammaSport, GammaSportsMarketTypes,
    GammaTeam, PolymarketEvent, PolymarketMarket, PolymarketSeries, PolymarketTag,
};
use serde_json::Value;

fn without(fixture: &str, pointer: &str, key: &str) -> String {
    let mut v: Value = serde_json::from_str(fixture).unwrap();
    v.pointer_mut(pointer)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(key)
        .unwrap();
    v.to_string()
}

const MARKETS_KEYSET: &str = include_str!("fixtures/gamma_markets_keyset.json");

#[test]
fn parses_markets_keyset_page() {
    let page: GammaMarketsPage = serde_json::from_str(MARKETS_KEYSET).unwrap();
    assert!(page.next_cursor.is_some());
    let m = &page.markets;
    assert_eq!(m.len(), 6);
    assert!(m[0].tags.is_some() && m[0].fee_schedule.is_some() && m[0].best_bid.is_some());
    assert!(m[0].events.as_ref().is_some_and(|e| !e.is_empty()));
    assert!(m[1].clob_rewards.is_some());
    assert!(m[2].closed_time.is_some() && m[2].lower_bound.is_some());
    assert!(m[3].line.is_some());
    // Old AMM-only market: no CLOB token ids, no parent events.
    assert!(m[4].clob_token_ids.is_none() && m[4].events.is_none());
    // `"feeType": null`.
    assert!(m[5].fee_type.is_none());

    let missing = without(MARKETS_KEYSET, "/markets/0", "conditionId");
    assert!(serde_json::from_str::<GammaMarketsPage>(&missing).is_err());
    let missing = without(MARKETS_KEYSET, "/markets/0/feeSchedule", "rate");
    assert!(serde_json::from_str::<GammaMarketsPage>(&missing).is_err());
}

#[test]
fn parses_single_market_routes() {
    let m: PolymarketMarket =
        serde_json::from_str(include_str!("fixtures/gamma_market.json")).unwrap();
    assert!(m.tags.is_some());
    let abridged: Vec<PolymarketMarket> =
        serde_json::from_str(include_str!("fixtures/gamma_markets_abridged.json")).unwrap();
    assert_eq!(abridged[0].id, m.id);
    let d: GammaMarketDescription =
        serde_json::from_str(include_str!("fixtures/gamma_market_description.json")).unwrap();
    assert!(d.description.is_some());
    let tags: Vec<PolymarketTag> =
        serde_json::from_str(include_str!("fixtures/gamma_market_tags.json")).unwrap();
    assert!(!tags.is_empty());
}

#[test]
fn parses_event_routes() {
    let e: PolymarketEvent =
        serde_json::from_str(include_str!("fixtures/gamma_event.json")).unwrap();
    assert_eq!(e.markets.len(), 1);
    let tags: Vec<PolymarketTag> =
        serde_json::from_str(include_str!("fixtures/gamma_event_tags.json")).unwrap();
    assert!(!tags.is_empty());
    let p: GammaEventsPagination =
        serde_json::from_str(include_str!("fixtures/gamma_events_pagination.json")).unwrap();
    assert_eq!(p.data.len(), 1);
    assert!(p.pagination.has_more);
}

#[test]
fn parses_series_routes() {
    let s: Vec<PolymarketSeries> =
        serde_json::from_str(include_str!("fixtures/gamma_series.json")).unwrap();
    assert_eq!(s.len(), 2);
    assert!(s[0].competitive.is_some());
    assert!(s[0].events.as_ref().is_some_and(|e| e.len() == 1));
    let summary = include_str!("fixtures/gamma_series_summary.json");
    let parsed: GammaSeriesSummary = serde_json::from_str(summary).unwrap();
    assert_eq!(parsed.event_dates.len(), parsed.event_weeks.len());
    assert!(
        serde_json::from_str::<GammaSeriesSummary>(&without(summary, "", "eventDates")).is_err()
    );
}

#[test]
fn parses_tag_routes() {
    let tags: Vec<PolymarketTag> =
        serde_json::from_str(include_str!("fixtures/gamma_tags.json")).unwrap();
    assert!(tags[0].created_at.is_none());
    assert!(tags[1].is_carousel.is_some());
    let tag: PolymarketTag = serde_json::from_str(include_str!("fixtures/gamma_tag.json")).unwrap();
    assert_eq!(tag.id, "1");
    let related = include_str!("fixtures/gamma_related_tags.json");
    let rows: Vec<GammaRelatedTag> = serde_json::from_str(related).unwrap();
    assert_eq!(rows[0].tag_id, 1);
    assert!(serde_json::from_str::<Vec<GammaRelatedTag>>(&without(related, "/0", "rank")).is_err());
    let related_tags: Vec<PolymarketTag> =
        serde_json::from_str(include_str!("fixtures/gamma_related_tag_tags.json")).unwrap();
    assert!(related_tags[0].active_events_count.is_some());
}

#[test]
fn parses_sports_routes() {
    let sports = include_str!("fixtures/gamma_sports.json");
    let rows: Vec<GammaSport> = serde_json::from_str(sports).unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        serde_json::from_str::<Vec<GammaSport>>(&without(sports, "/0", "primaryTagId")).is_err()
    );
    let types: GammaSportsMarketTypes =
        serde_json::from_str(include_str!("fixtures/gamma_sports_market_types.json")).unwrap();
    assert!(!types.market_types.is_empty());

    let teams = include_str!("fixtures/gamma_teams.json");
    let rows: Vec<GammaTeam> = serde_json::from_str(teams).unwrap();
    assert!(rows[0].alias.is_some());
    assert!(rows[1].record.is_none() && rows[1].updated_at.is_none());
    assert!(serde_json::from_str::<Vec<GammaTeam>>(&without(teams, "/0", "league")).is_err());
    let _: GammaTeam = serde_json::from_str(include_str!("fixtures/gamma_team.json")).unwrap();
}

#[test]
fn parses_search_and_errors() {
    let s: GammaSearch = serde_json::from_str(include_str!("fixtures/gamma_search.json")).unwrap();
    assert!(s.events.as_ref().is_some_and(|e| !e.is_empty()));
    assert!(s.tags.as_ref().is_some_and(|t| t[0].event_count > 0));
    let e: GammaError = serde_json::from_str(include_str!("fixtures/gamma_error.json")).unwrap();
    assert_eq!(e.error, "id not found");
}

#[test]
fn parses_live_events_keyset_page() {
    let page: EventsPage =
        serde_json::from_str(include_str!("fixtures/gamma_events_keyset.json")).unwrap();
    assert!(page.next_cursor.is_some());

    let ids: Vec<_> = page.events.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["28799", "34053", "578465"]);

    let negative_comments = &page.events[0];
    assert_eq!(negative_comments.comment_count, Some(-1));

    let with_series = &page.events[1];
    assert!(with_series.series.as_ref().is_some_and(|s| !s.is_empty()));
    assert_eq!(with_series.markets.len(), 2);

    let without_tags = &page.events[2];
    assert!(without_tags.tags.is_none());
    assert!(without_tags.series.is_none());

    for market in page.events.iter().flat_map(|e| &e.markets) {
        let outcomes = market.outcomes.as_ref().unwrap();
        assert!(outcomes.len() >= 2);
        if let Some(tokens) = &market.clob_token_ids {
            assert_eq!(tokens.len(), outcomes.len());
        }
    }
}
