//! App Store Connect App Asset Library calls against a wiremock server
//! (no real network). Response shapes follow Apple's documentation examples
//! (ADR 0019).

mod common;

use arrangia_publish::apple::library::{
    CATEGORY_CREATIVE, CATEGORY_SCREENSHOTS, RefDataFilter, is_asset_has_placements,
};
use arrangia_publish::apple::{Client, Config, LibraryUpload};
use arrangia_publish::{Error, UploadStep, ignore_progress};
use common::{ProgressLog, ec_key, fast_options};
use serde_json::{Value, json};
use wiremock::matchers::{body_bytes, body_json, body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await;
    let (pem, _) = ec_key();
    let config = Config::team("issuer", "KID", pem).with_base_url(server.uri());
    (server, Client::new(config, fast_options()).unwrap())
}

/// Apple's documented ref data example plus a Duo group and a header type.
pub fn ref_data_body() -> Value {
    json!({
        "data": [{
            "type": "appAssetLibraryRefData",
            "id": "1",
            "attributes": {
                "features": [{
                    "featureId": "APP_STORE_VERSIONS",
                    "placementPolicies": [
                        { "placementType": "APP_SCREENSHOT",
                          "groupLimits": [{ "groupIds": ["IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE", "MAC_PROFILE"], "maxCount": 10 }] },
                        { "placementType": "PRODUCT_PAGE_HEADER_ASSET",
                          "groupLimits": [{ "groupIds": ["IPHONE_HEADER_PROFILE"], "maxCount": 1 }] }
                    ]
                }],
                "placementProfileGroups": [
                    { "placementProfileGroupId": "IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE",
                      "platform": "IPHONE_APP_STORE", "displayClassId": "IPHONE_DYNAMIC_ISLAND_LARGE_DISPLAY" },
                    { "placementProfileGroupId": "IPHONE_HEADER_PROFILE",
                      "platform": "IPHONE_APP_STORE", "displayClassId": "DEFAULT" }
                ],
                "imageSpecs": [{
                    "specId": "f56c777c-99ea-5760-97ae-27c5f8cbb884",
                    "shortName": "i1290x2796a0",
                    "dimensions": { "minWidth": 1290, "maxWidth": 1290, "minHeight": 2796, "maxHeight": 2796 },
                    "aspectRatio": "6:13",
                    "compatiblePlacementTypes": ["APP_SCREENSHOT", "IMESSAGE_APP_SCREENSHOT"],
                    "alphaAllowed": false,
                    "fileExtensions": [".jpg", ".jpeg", ".png"],
                    "maxFileSize": 524288000,
                    "mimeTypes": ["image/jpeg", "image/png"],
                    "universalAsset": false
                }],
                "videoSpecs": [{
                    "specId": "1861fdcb-eb99-59e6-8c6c-5a07479d9a84",
                    "shortName": "v886x1920f23~30t15~30u1",
                    "dimensions": { "minWidth": 886, "maxWidth": 886, "minHeight": 1920, "maxHeight": 1920 },
                    "compatiblePlacementTypes": ["APP_PREVIEW"],
                    "frameRates": [{ "minFps": 23, "maxFps": 30 }],
                    "duration": { "min": "PT15S", "max": "PT30S" },
                    "audioRequired": true,
                    "universalAsset": false
                }],
                "placementTypes": [
                    { "placementTypeId": "APP_SCREENSHOT",
                      "acceptsAssetCategories": ["APP_SCREENSHOTS_AND_PREVIEWS"],
                      "specMappings": [{ "placementGroupId": "IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE",
                                         "specs": ["f56c777c-99ea-5760-97ae-27c5f8cbb884"] }] },
                    { "placementTypeId": "PRODUCT_PAGE_HEADER_ASSET",
                      "acceptsAssetCategories": ["CREATIVE_ASSETS"],
                      "specMappings": [{ "placementGroupId": "IPHONE_HEADER_PROFILE", "specs": [] }] }
                ],
                "displayClasses": [{
                    "displayClassId": "IPHONE_DYNAMIC_ISLAND_LARGE_DISPLAY",
                    "deviceFamily": "IPHONE",
                    "screenDimensions": ["6.9"]
                }]
            },
            "links": { "self": "https://api.appstoreconnect.apple.com/v1/appAssetLibraryRefData/1" }
        }],
        "links": { "self": "https://api.appstoreconnect.apple.com/v1/appAssetLibraryRefData" },
        "meta": { "paging": { "total": 1, "limit": 50 } }
    })
}

fn image(id: &str, state: &str, ops: &Value) -> Value {
    json!({
        "data": {
            "type": "appAssetLibraryImages",
            "id": id,
            "attributes": {
                "category": "APP_SCREENSHOTS_AND_PREVIEWS",
                "fileName": "01.png",
                "fileSize": 10,
                "imageAsset": null,
                "referenceName": "01 [abc]",
                "specId": null,
                "state": state,
                "stateDetails": null,
                "uploadOperations": ops
            }
        }
    })
}

#[tokio::test]
async fn library_and_ref_data_are_parsed() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/apps/1234567890/assetLibrary"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "type": "appAssetLibraries", "id": "lib-1" }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/appAssetLibraryRefData"))
        .and(query_param("filter[features]", "APP_STORE_VERSIONS"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ref_data_body()))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        client.asset_library("1234567890").await.unwrap().id,
        "lib-1"
    );
    let rd = client
        .ref_data(&RefDataFilter {
            features: vec!["APP_STORE_VERSIONS".into()],
            ..RefDataFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(rd.placement_profile_groups.len(), 2);
    let platforms = vec!["IPHONE_APP_STORE".to_owned()];
    let groups = rd.groups_for(
        "APP_SCREENSHOT",
        Some("IPHONE_DYNAMIC_ISLAND_LARGE_DISPLAY"),
        &platforms,
    );
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].placement_profile_group_id,
        "IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE"
    );
    let specs = rd.image_specs_for("APP_SCREENSHOT", "IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE");
    assert!(specs[0].accepts_size(1290, 2796));
    assert_eq!(
        rd.max_count(
            "APP_STORE_VERSIONS",
            "APP_SCREENSHOT",
            "IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE"
        ),
        Some(10)
    );
    assert!(
        rd.groups_for("APP_SCREENSHOT", Some("IPHONE_DUO"), &platforms)
            .is_empty()
    );
    assert_eq!(
        rd.groups_for("PRODUCT_PAGE_HEADER_ASSET", None, &platforms)
            .len(),
        1
    );
    assert_eq!(rd.video_specs[0].dimensions.max_height, 1920);
    // Unknown fields (frameRates, duration) do not break parsing; the dump
    // keeps Apple's names.
    let dumped = serde_json::to_value(&rd).unwrap();
    assert!(dumped["placementProfileGroups"][0]["displayClassId"].is_string());
}

#[tokio::test]
async fn image_upload_reserves_transfers_and_commits_without_checksum() {
    let (server, client) = fixture().await;
    let ops = json!([{
        "method": "PUT",
        "url": format!("{}/blob/part1?sig=SECRET", server.uri()),
        "length": 10, "offset": 0,
        "requestHeaders": [{ "name": "Content-Type", "value": "image/png" }]
    }]);
    Mock::given(method("POST"))
        .and(path("/v1/appAssetLibraryImages"))
        .and(body_json(json!({
            "data": {
                "type": "appAssetLibraryImages",
                "attributes": { "fileName": "01.png", "fileSize": 10,
                                "category": "APP_SCREENSHOTS_AND_PREVIEWS", "referenceName": "01 [abc]" },
                "relationships": { "assetLibrary": { "data": { "type": "appAssetLibraries", "id": "lib-1" } } }
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(image("img-1", "AWAITING_UPLOAD", &ops)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/blob/part1"))
        .and(body_bytes(b"\x89PNG012345".to_vec()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    // Exactly `{uploaded: true}` – no sourceFileChecksum.
    Mock::given(method("PATCH"))
        .and(path("/v1/appAssetLibraryImages/img-1"))
        .and(body_json(json!({
            "data": { "type": "appAssetLibraryImages", "id": "img-1", "attributes": { "uploaded": true } }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(image("img-1", "UPLOAD_COMPLETE", &json!(null))))
        .expect(1)
        .mount(&server)
        .await;
    let log = ProgressLog::default();
    let img = client
        .upload_library_image_committed(
            "lib-1",
            &LibraryUpload {
                file_name: "01.png",
                category: CATEGORY_SCREENSHOTS,
                reference_name: Some("01 [abc]"),
            },
            b"\x89PNG012345".to_vec(),
            &|p| log.push(p),
        )
        .await
        .unwrap();
    assert_eq!(img.id, "img-1");
    assert!(img.is_processing() && img.is_usable() && !img.is_ready());
    let steps: Vec<UploadStep> = log.all().into_iter().map(|p| p.step).collect();
    assert_eq!(steps.first(), Some(&UploadStep::Reserve));
    assert_eq!(steps.last(), Some(&UploadStep::Commit));
    assert_eq!(CATEGORY_CREATIVE, "CREATIVE_ASSETS");
}

#[tokio::test]
async fn processed_and_failed_images() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/appAssetLibraryImages/ok"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "type": "appAssetLibraryImages", "id": "ok", "attributes": {
                "state": "PREPARE_FOR_SUBMISSION", "specId": "f56c",
                "imageAsset": { "templateUrl": "https://x/{w}x{h}bb.{f}", "width": 1290, "height": 2796 } } }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/appAssetLibraryImages/bad"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "type": "appAssetLibraryImages", "id": "bad", "attributes": {
                "state": "FAILED",
                "stateDetails": [{ "code": "IMAGE_TOOL_FAILURE", "description": "Invalid dimensions" }] } }
        })))
        .mount(&server)
        .await;
    let ok = client.library_image("ok").await.unwrap();
    assert!(ok.is_ready());
    assert_eq!((ok.width, ok.height), (Some(1290), Some(2796)));
    assert_eq!(ok.spec_id.as_deref(), Some("f56c"));
    let bad = client.library_image("bad").await.unwrap();
    assert!(bad.is_failed() && !bad.is_usable());
    assert_eq!(bad.state_details[0].detail, "Invalid dimensions");
}

#[tokio::test]
async fn placements_create_list_order_delete() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/appAssetLibraryPlacements"))
        .and(body_json(json!({
            "data": {
                "type": "appAssetLibraryPlacements",
                "attributes": { "placementType": "APP_SCREENSHOT",
                                "placementGroup": "IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE" },
                "relationships": {
                    "image": { "data": { "type": "appAssetLibraryImages", "id": "img-1" } },
                    "appStoreVersionLocalization": { "data": { "type": "appStoreVersionLocalizations", "id": "loc-1" } }
                }
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "data": { "type": "appAssetLibraryPlacements", "id": "pl-1", "attributes": {
                "mediaType": "IMAGE", "placementType": "APP_SCREENSHOT",
                "placementGroup": "IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE",
                "state": "PARENT_PREPARE_FOR_SUBMISSION", "stateDetails": null } }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/appStoreVersionLocalizations/loc-1/placements"))
        .and(query_param("sort", "placementGroupPosition"))
        .and(query_param("include", "image"))
        .and(query_param("filter[placementType]", "APP_SCREENSHOT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                { "type": "appAssetLibraryPlacements", "id": "pl-2",
                  "attributes": { "placementType": "APP_SCREENSHOT", "placementGroup": "G", "state": "ASSET_PROCESSING" },
                  "relationships": { "image": { "data": { "type": "appAssetLibraryImages", "id": "img-2" } } } },
                { "type": "appAssetLibraryPlacements", "id": "pl-1",
                  "attributes": { "placementType": "APP_SCREENSHOT", "placementGroup": "G" },
                  "relationships": { "image": { "data": { "type": "appAssetLibraryImages", "id": "img-1" } } } }
            ],
            "included": [
                { "type": "appAssetLibraryImages", "id": "img-1",
                  "attributes": { "referenceName": "a [111]", "fileSize": 10, "state": "PREPARE_FOR_SUBMISSION" } },
                { "type": "appAssetLibraryImages", "id": "img-2",
                  "attributes": { "referenceName": "b [222]", "fileSize": 12, "state": "UPLOAD_COMPLETE" } }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/appAssetLibraryPlacementOrderingRequests"))
        .and(body_partial_json(json!({
            "data": {
                "type": "appAssetLibraryPlacementOrderingRequests",
                "attributes": { "placementGroup": "G" },
                "relationships": {
                    "orderedPlacements": { "data": [
                        { "type": "appAssetLibraryPlacements", "id": "pl-1" },
                        { "type": "appAssetLibraryPlacements", "id": "pl-2" } ] },
                    "appStoreVersionLocalization": { "data": { "type": "appStoreVersionLocalizations", "id": "loc-1" } }
                }
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "data": { "type": "appAssetLibraryPlacementOrderingRequests", "id": "or-1" }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/appAssetLibraryPlacements/pl-2"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/appAssetLibraryPlacements/gone"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let p = client
        .create_placement(
            "APP_SCREENSHOT",
            "IPHONE_DYNAMIC_ISLAND_LARGE_PROFILE",
            "img-1",
            "loc-1",
        )
        .await
        .unwrap();
    assert_eq!(p.id, "pl-1");
    assert_eq!(p.state.as_deref(), Some("PARENT_PREPARE_FOR_SUBMISSION"));
    let list = client
        .localization_placements("loc-1", Some("APP_SCREENSHOT"), None)
        .await
        .unwrap();
    let ids: Vec<&str> = list.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["pl-2", "pl-1"]);
    assert_eq!(list[0].image_id.as_deref(), Some("img-2"));
    let img = list[0].image.as_ref().unwrap();
    assert_eq!(img.reference_name.as_deref(), Some("b [222]"));
    assert!(img.is_processing());
    client
        .order_placements("G", "loc-1", &["pl-1".into(), "pl-2".into()])
        .await
        .unwrap();
    client.delete_placement("pl-2").await.unwrap();
    client.delete_placement("gone").await.unwrap();
}

#[tokio::test]
async fn deleting_an_asset_with_placements_is_a_conflict() {
    let (server, client) = fixture().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/appAssetLibraryImages/used"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "errors": [{ "status": "409", "code": "STATE_ERROR.ASSET_HAS_PLACEMENTS",
                         "title": "The asset cannot be deleted",
                         "detail": "The asset cannot be deleted while placements still use it. Delete those placements first." }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/appAssetLibraryImages/free"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let e = client.delete_library_image("used").await.unwrap_err();
    assert!(matches!(e, Error::Conflict { .. }), "{e:?}");
    assert!(is_asset_has_placements(&e));
    client.delete_library_image("free").await.unwrap();
}

#[tokio::test]
async fn failed_transfer_deletes_the_reservation() {
    let (server, client) = fixture().await;
    let ops = json!([{
        "method": "PUT", "url": format!("{}/blob/p", server.uri()), "length": 10, "offset": 0,
        "requestHeaders": []
    }]);
    Mock::given(method("POST"))
        .and(path("/v1/appAssetLibraryImages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(image(
            "img-9",
            "AWAITING_UPLOAD",
            &ops,
        )))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/blob/p"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/appAssetLibraryImages/img-9"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let e = client
        .upload_library_image_committed(
            "lib-1",
            &LibraryUpload {
                file_name: "01.png",
                category: CATEGORY_SCREENSHOTS,
                reference_name: None,
            },
            b"\x89PNG012345".to_vec(),
            &ignore_progress,
        )
        .await
        .unwrap_err();
    assert!(matches!(e, Error::Validation { .. }), "{e:?}");
}
