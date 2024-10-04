// These tests requires a running SPIRE server and agent with workloads registered (see script `scripts/run-spire.sh`).
// In addition it requires the admin endpoint to be exposed, and the running user to registered
// as an authorized_delegate.

#[cfg(feature = "integration-tests")]
mod integration_tests_delegate_identity_api_client {
    use once_cell::sync::Lazy;
    use spiffe::bundle::BundleRefSource;
    use spiffe::{JwtBundleSet, TrustDomain};
    use spire_api::{selectors, DelegateAttestationRequest, DelegatedIdentityClient};
    use std::process::Command;
    use tokio_stream::StreamExt;

    static TRUST_DOMAIN: Lazy<TrustDomain> = Lazy::new(|| TrustDomain::new("example.org").unwrap());

    fn get_uid() -> u16 {
        let mut uid = String::from_utf8(
            Command::new("id")
                .arg("-u")
                .output()
                .expect("could not get UID")
                .stdout,
        )
        .expect("could not parse to string");
        uid.truncate(uid.len() - 1);
        uid.parse().expect("could not parse uid to number")
    }

    async fn get_client() -> DelegatedIdentityClient {
        DelegatedIdentityClient::default()
            .await
            .expect("failed to create client")
    }

    #[tokio::test]
    async fn fetch_delegate_jwt_svid() {
        let mut client = get_client().await;
        let svid = client
            .fetch_jwt_svids(
                &["my_audience"],
                DelegateAttestationRequest::Selectors(vec![selectors::Selector::Unix(
                    selectors::Unix::Uid(get_uid() + 1),
                )]),
            )
            .await
            .expect("Failed to fetch JWT SVID");
        assert_eq!(svid.len(), 1);
        assert_eq!(svid[0].audience(), &["my_audience"]);
    }

    #[tokio::test]
    async fn fetch_delegate_x509_svid() {
        let mut client = get_client().await;
        let response: spiffe::svid::x509::X509Svid = client
            .fetch_x509_svid(DelegateAttestationRequest::Selectors(vec![
                selectors::Selector::Unix(selectors::Unix::Uid(get_uid() + 1)),
            ]))
            .await
            .expect("Failed to fetch delegate SVID");
        // Not checking the chain as the root is generated by spire.
        // In the future we could look in the downloaded spire directory for the keys.
        assert_eq!(response.cert_chain().len(), 1);
        assert_eq!(
            response.spiffe_id().to_string(),
            "spiffe://example.org/different-process"
        );
    }

    #[tokio::test]
    async fn stream_delegate_x509_svid() {
        let test_duration = std::time::Duration::from_secs(60);
        let mut client = get_client().await;
        let mut stream = client
            .stream_x509_svids(DelegateAttestationRequest::Selectors(vec![
                selectors::Selector::Unix(selectors::Unix::Uid(get_uid() + 1)),
            ]))
            .await
            .expect("Failed to fetch delegate SVID");

        let result = tokio::time::timeout(test_duration, stream.next())
            .await
            .expect("Test did not complete in the expected duration");
        let response = result.expect("empty result").expect("error in stream");
        // Not checking the chain as the root is generated by spire.
        // In the future we could look in the downloaded spire directory for the keys.
        assert_eq!(response.cert_chain().len(), 1);
        assert_eq!(
            response.spiffe_id().to_string(),
            "spiffe://example.org/different-process"
        );
    }

    #[tokio::test]
    async fn fetch_delegated_x509_trust_bundles() {
        let mut client = get_client().await;
        let response = client
            .fetch_x509_bundles()
            .await
            .expect("Failed to fetch trust bundles");
        response
            .get_bundle(&*TRUST_DOMAIN)
            .expect("Failed to get bundle");
    }

    #[tokio::test]
    async fn stream_delegated_x509_trust_bundles() {
        let test_duration = std::time::Duration::from_secs(60);
        let mut client = get_client().await;
        let mut stream = client
            .stream_x509_bundles()
            .await
            .expect("Failed to fetch trust bundles");

        let result = tokio::time::timeout(test_duration, stream.next())
            .await
            .expect("Test did not complete in the expected duration");
        let response = result.expect("empty result").expect("error in stream");
        response
            .get_bundle(&*TRUST_DOMAIN)
            .expect("Failed to get bundle");
    }

    async fn verify_jwt(client: &mut DelegatedIdentityClient, bundles: JwtBundleSet) {
        let svids = client
            .fetch_jwt_svids(
                &["my_audience"],
                DelegateAttestationRequest::Selectors(vec![selectors::Selector::Unix(
                    selectors::Unix::Uid(get_uid() + 1),
                )]),
            )
            .await
            .expect("Failed to fetch JWT SVID");
        let svid = svids.first().expect("no items in jwt bundle list");
        let key_id = svid.key_id();

        let bundle = bundles.get_bundle_for_trust_domain(&*TRUST_DOMAIN);
        let bundle = bundle
            .expect("Bundle was None")
            .expect("Failed to unwrap bundle");
        assert_eq!(bundle.trust_domain(), &*TRUST_DOMAIN);
        assert_eq!(
            bundle.find_jwt_authority(key_id).unwrap().common.key_id,
            Some(key_id.to_string())
        );
    }

    #[tokio::test]
    async fn fetch_delegated_jwt_trust_bundles() {
        let mut client = get_client().await;
        let response = client
            .fetch_jwt_bundles()
            .await
            .expect("Failed to fetch trust bundles");

        verify_jwt(&mut client, response).await;
    }

    #[tokio::test]
    async fn stream_delegated_jwt_trust_bundles() {
        let mut client = get_client().await;
        let test_duration = std::time::Duration::from_secs(60);
        let mut stream = client
            .stream_jwt_bundles()
            .await
            .expect("Failed to fetch trust bundles");

        let result = tokio::time::timeout(test_duration, stream.next())
            .await
            .expect("Test did not complete in the expected duration");

        verify_jwt(
            &mut client,
            result.expect("empty result").expect("error in stream"),
        )
        .await;
    }
}
