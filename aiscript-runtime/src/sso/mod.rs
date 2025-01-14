use openidconnect::{
    core::{CoreClient, CoreProviderMetadata, CoreResponseType},
    AuthenticationFlow, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, RedirectUrl, Scope,
};
use reqwest::ClientBuilder;

type Client<
    HasAuthUrl = EndpointSet,
    HasDeviceAuthUrl = EndpointNotSet,
    HasIntrospectionUrl = EndpointNotSet,
    HasRevocationUrl = EndpointNotSet,
    HasTokenUrl = EndpointMaybeSet,
    HasUserInfoUrl = EndpointMaybeSet,
> = CoreClient<
    HasAuthUrl,
    HasDeviceAuthUrl,
    HasIntrospectionUrl,
    HasRevocationUrl,
    HasTokenUrl,
    HasUserInfoUrl,
>;

pub struct Provider {
    // client_id: ClientId,
    // client_secret: ClientSecret,
    // redirect_uri: String,
    http_client: reqwest::Client,
    client: Client,
}

impl Provider {
    pub async fn new(
        issuer_url: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Self {
        let client_id = ClientId::new(client_id.to_owned());
        let client_secret = ClientSecret::new(client_secret.to_owned());
        let http_client = ClientBuilder::new()
            // Following redirects opens the client up to SSRF vulnerabilities.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let provider_metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(issuer_url.to_owned()).unwrap(),
            &http_client,
        )
        .await
        .unwrap();

        // Set up the config for the OAuth2 process.
        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
                // This example will be running its own server at localhost:8080.
                // See below for the server implementation.
                .set_redirect_uri(RedirectUrl::new(redirect_uri.to_owned()).unwrap());
        Provider {
            http_client,
            client,
        }
    }

    // Generate the authorization URL to which we'll redirect the user.
    fn authorize_url(&self) -> String {
        let (authorize_url, _csrf_state, _nonce) = self
            .client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            // This example is requesting access to the the user's profile including email.
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();

        authorize_url.to_string()
    }
}
