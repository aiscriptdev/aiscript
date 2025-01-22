use crate::{
    object::Class,
    value::Value,
    vm::{Context, State},
    VmError,
};
use gc_arena::{Gc, GcRefLock, RefLock};
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, TokenUrl,
};

pub fn create_sso_provider_class(ctx: Context) -> GcRefLock<'_, Class> {
    let mut sso_provider_class = Class::new(ctx.intern(b"SsoProvider"));
    sso_provider_class.methods.insert(
        ctx.intern(b"authority_url"),
        Value::NativeFunction(crate::NativeFn(authority_url)),
    );
    Gc::new(&ctx, RefLock::new(sso_provider_class))
}

type SsoProviderClient<
    HasAuthUrl = EndpointSet,
    HasDeviceAuthUrl = EndpointNotSet,
    HasIntrospectionUrl = EndpointNotSet,
    HasRevocationUrl = EndpointNotSet,
    HasTokenUrl = EndpointSet,
> = BasicClient<HasAuthUrl, HasDeviceAuthUrl, HasIntrospectionUrl, HasRevocationUrl, HasTokenUrl>;

fn get_client(
    client_id: &str,
    client_secret: &str,
    auth_url: &str,
    token_url: &str,
    redirect_url: &str,
) -> SsoProviderClient {
    let auth_url = AuthUrl::new(auth_url.to_owned()).expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new(token_url.to_owned()).expect("Invalid token endpoint URL");

    BasicClient::new(ClientId::new(client_id.to_owned()))
        .set_client_secret(ClientSecret::new(client_secret.to_owned()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(RedirectUrl::new(redirect_url.to_owned()).expect("Invalid redirect URL"))
}

fn authority_url<'gc>(
    state: &mut State<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
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
        let auth_url = fields
            .get(&state.intern_static("auth_url"))
            .unwrap()
            .as_string()
            .unwrap()
            .to_string();
        let token_url = fields
            .get(&state.intern_static("token_url"))
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

        // Generate the authorization URL to which we'll redirect the user.
        let (authorize_url, _csrf_state) = get_client(
            &client_id,
            &client_secret,
            &auth_url,
            &token_url,
            &redirect_url,
        )
        .authorize_url(CsrfToken::new_random)
        .add_scopes(scopes)
        .url();

        Ok(Value::String(
            state.intern(authorize_url.as_str().as_bytes()),
        ))
    } else {
        Err(VmError::RuntimeError("Invalid receiver".into()))
    }
}
