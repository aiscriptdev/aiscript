use std::mem;

use crate::{
    VmError,
    object::Class,
    value::Value,
    vm::{Context, State},
};
use aiscript_arena::{Gc, GcRefLock, RefLock};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, TokenResponse, TokenUrl, basic::BasicClient,
};
use tokio::runtime::Handle;

pub fn create_sso_provider_class(ctx: Context) -> GcRefLock<'_, Class> {
    let mut sso_provider_class = Class::new(ctx.intern(b"SsoProvider"));
    sso_provider_class.methods = [
        (
            ctx.intern(b"authority_url"),
            Value::NativeFunction(crate::NativeFn(authority_url)),
        ),
        (
            ctx.intern(b"verify"),
            Value::NativeFunction(crate::NativeFn(verify)),
        ),
    ]
    .into_iter()
    .collect();
    Gc::new(&ctx, RefLock::new(sso_provider_class))
}

type SsoProviderClient<
    HasAuthUrl = EndpointSet,
    HasDeviceAuthUrl = EndpointNotSet,
    HasIntrospectionUrl = EndpointNotSet,
    HasRevocationUrl = EndpointNotSet,
    HasTokenUrl = EndpointSet,
> = BasicClient<HasAuthUrl, HasDeviceAuthUrl, HasIntrospectionUrl, HasRevocationUrl, HasTokenUrl>;

struct AuthFields {
    client_id: String,
    client_secret: String,
    auth_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
    redirect_url: String,
    scopes: Vec<Scope>,
}

fn parse_auth_fields(state: &mut State<'_>) -> Result<AuthFields, VmError> {
    if let Value::Instance(receiver) = state.peek(0) {
        let fields = &receiver.borrow().fields;
        let client_id = fields
            .get(&state.intern_static("client_id"))
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        let client_secret = fields
            .get(&state.intern_static("client_secret"))
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        let auth_endpoint = fields
            .get(&state.intern_static("auth_endpoint"))
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        let token_endpoint = fields
            .get(&state.intern_static("token_endpoint"))
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        let userinfo_endpoint = fields
            .get(&state.intern_static("userinfo_endpoint"))
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        let redirect_url = fields
            .get(&state.intern_static("redirect_url"))
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        let scopes = fields
            .get(&state.intern_static("scopes"))
            .unwrap()
            .as_array()
            .unwrap()
            .borrow()
            .data
            .iter()
            .map(|scope| Scope::new(scope.as_string().unwrap().to_string()))
            .collect::<Vec<_>>();

        Ok(AuthFields {
            client_id,
            client_secret,
            auth_endpoint,
            token_endpoint,
            userinfo_endpoint,
            redirect_url,
            scopes,
        })
    } else {
        Err(VmError::RuntimeError("Invalid receiver".into()))
    }
}

fn get_client(fields: AuthFields) -> SsoProviderClient {
    let auth_endpoint =
        AuthUrl::new(fields.auth_endpoint).expect("Invalid authorization endpoint URL");
    let token_endpoint = TokenUrl::new(fields.token_endpoint).expect("Invalid token endpoint URL");

    BasicClient::new(ClientId::new(fields.client_id))
        .set_client_secret(ClientSecret::new(fields.client_secret))
        .set_auth_uri(auth_endpoint)
        .set_token_uri(token_endpoint)
        .set_redirect_uri(RedirectUrl::new(fields.redirect_url).expect("Invalid redirect URL"))
}

fn authority_url<'gc>(
    state: &mut State<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let mut fields = parse_auth_fields(state)?;
    let scopes = mem::take(&mut fields.scopes);
    // Generate the authorization URL to which we'll redirect the user.
    let (authorize_url, _csrf_state) = get_client(fields)
        .authorize_url(CsrfToken::new_random)
        .add_scopes(scopes)
        .url();

    Ok(Value::String(
        state.intern(authorize_url.as_str().as_bytes()),
    ))
}

fn verify<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let mut i = 0;
    let mut code = None;

    while i < args.len() {
        match &args[i] {
            Value::String(key) if i + 1 < args.len() => match key.to_str().unwrap() {
                "code" => {
                    code = Some(args[i + 1].as_string()?);
                    i += 2;
                }
                _ => {
                    return Err(VmError::RuntimeError(format!(
                        "Unknown keyword argument: {}",
                        key
                    )));
                }
            },
            _ => {
                return Err(VmError::RuntimeError(
                    "verify() requires keyword arguments (e.g., verify(code=\"abc\"))".into(),
                ));
            }
        }
    }

    let code = code
        .ok_or_else(|| VmError::RuntimeError("verify() requires 'code' keyword argument".into()))?;

    let mut fields = parse_auth_fields(state)?;
    let userinfo_endpoint = mem::take(&mut fields.userinfo_endpoint);

    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    Handle::current().block_on(async {
        let token = get_client(fields)
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .request_async(&http_client)
            .await
            .map_err(|err| VmError::RuntimeError(err.to_string()))?;
        let access_token = token.access_token().secret().to_owned();

        let response = http_client
            .get(userinfo_endpoint)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|err| VmError::RuntimeError(err.to_string()))?;

        if !response.status().is_success() {
            return Err(VmError::RuntimeError(format!(
                "Failed to fetch user info: {}",
                response.status()
            )));
        }

        let info = response
            .json::<serde_json::Value>()
            .await
            .map_err(|err| VmError::RuntimeError(err.to_string()))?;
        Ok(Value::from_serde_value(state.get_context(), &info))
    })
}
