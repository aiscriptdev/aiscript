use std::collections::HashMap;

use gc_arena::{Gc, GcRefLock, RefLock};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};

use crate::{
    module::ModuleKind,
    object::Object,
    vm::{Context, State},
    NativeFn, Value, VmError,
};

#[derive(Debug, Serialize, Deserialize, Default)]
struct Claims {
    // Standard claims
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>, // Issuer
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<String>, // Subject
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>, // Audience
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<i64>, // Expiration Time
    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<i64>, // Not Before
    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<i64>, // Issued At
    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<String>, // JWT ID
    // Custom claims stored as extra fields
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

pub fn create_jwt_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.auth.jwt");
    let exports = [
        ("encode", Value::NativeFunction(NativeFn(jwt_encode))),
        ("decode", Value::NativeFunction(NativeFn(jwt_decode))),
        ("HS256", Value::String(ctx.intern(b"HS256"))),
        ("HS384", Value::String(ctx.intern(b"HS384"))),
        ("HS512", Value::String(ctx.intern(b"HS512"))),
        ("RS256", Value::String(ctx.intern(b"RS256"))),
        ("RS384", Value::String(ctx.intern(b"RS384"))),
        ("RS512", Value::String(ctx.intern(b"RS512"))),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();

    ModuleKind::Native { name, exports }
}

fn parse_algorithm(alg: &str) -> Result<Algorithm, VmError> {
    match alg {
        "HS256" => Ok(Algorithm::HS256),
        "HS384" => Ok(Algorithm::HS384),
        "HS512" => Ok(Algorithm::HS512),
        "RS256" => Ok(Algorithm::RS256),
        "RS384" => Ok(Algorithm::RS384),
        "RS512" => Ok(Algorithm::RS512),
        _ => Err(VmError::RuntimeError(format!(
            "Unsupported algorithm: {}",
            alg
        ))),
    }
}

fn build_claims<'gc>(claims_obj: &GcRefLock<'gc, Object<'gc>>) -> Result<Claims, VmError> {
    let mut claims = Claims::default();
    let fields = &claims_obj.borrow().fields;
    for (key, value) in fields {
        let key_str = key.to_str().unwrap();
        match key_str {
            "iss" => claims.iss = Some(value.as_string()?.to_string()),
            "sub" => claims.sub = Some(value.as_string()?.to_string()),
            "aud" => claims.aud = Some(value.as_string()?.to_string()),
            "exp" => claims.exp = Some(value.as_number()? as i64),
            "nbf" => claims.nbf = Some(value.as_number()? as i64),
            "iat" => claims.iat = Some(value.as_number()? as i64),
            "jti" => claims.jti = Some(value.as_string()?.to_string()),
            // Handle custom claims
            _ => {
                let json_value = match value {
                    Value::String(s) => serde_json::Value::String(s.to_string()),
                    Value::Number(n) => {
                        serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap())
                    }
                    Value::Boolean(b) => serde_json::Value::Bool(*b),
                    Value::Nil => serde_json::Value::Null,
                    _ => {
                        return Err(VmError::RuntimeError(format!(
                            "Unsupported claim value type for key: {}",
                            key_str
                        )))
                    }
                };
                claims.extra.insert(key_str.to_string(), json_value);
            }
        }
    }

    Ok(claims)
}

fn claims_to_object(ctx: Context, claims: Claims) -> Value {
    let mut obj = Object::default();

    // Add standard claims
    if let Some(iss) = claims.iss {
        obj.fields.insert(
            ctx.intern(b"iss"),
            Value::String(ctx.intern(iss.as_bytes())),
        );
    }
    if let Some(sub) = claims.sub {
        obj.fields.insert(
            ctx.intern(b"sub"),
            Value::String(ctx.intern(sub.as_bytes())),
        );
    }
    if let Some(aud) = claims.aud {
        obj.fields.insert(
            ctx.intern(b"aud"),
            Value::String(ctx.intern(aud.as_bytes())),
        );
    }
    if let Some(exp) = claims.exp {
        obj.fields
            .insert(ctx.intern(b"exp"), Value::Number(exp as f64));
    }
    if let Some(nbf) = claims.nbf {
        obj.fields
            .insert(ctx.intern(b"nbf"), Value::Number(nbf as f64));
    }
    if let Some(iat) = claims.iat {
        obj.fields
            .insert(ctx.intern(b"iat"), Value::Number(iat as f64));
    }
    if let Some(jti) = claims.jti {
        obj.fields.insert(
            ctx.intern(b"jti"),
            Value::String(ctx.intern(jti.as_bytes())),
        );
    }

    // Add custom claims
    for (key, value) in claims.extra {
        let value = match value {
            serde_json::Value::String(s) => Value::String(ctx.intern(s.as_bytes())),
            serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::Bool(b) => Value::Boolean(b),
            serde_json::Value::Null => Value::Nil,
            _ => continue, // Skip unsupported types
        };
        obj.fields.insert(ctx.intern(key.as_bytes()), value);
    }

    Value::Object(Gc::new(&ctx, RefLock::new(obj)))
}

fn jwt_encode<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 3 {
        return Err(VmError::RuntimeError(
            "encode() requires claims object, secret key, and algorithm".into(),
        ));
    }

    // Get claims object
    let claims_obj = match &args[0] {
        Value::Object(obj) => obj,
        _ => {
            return Err(VmError::RuntimeError(
                "First argument must be claims object".into(),
            ))
        }
    };

    // Get secret key
    let secret = args[1].as_string()?;
    let secret_str = secret.to_str().unwrap();

    // Get algorithm
    let alg = args[2].as_string()?;
    let algorithm = parse_algorithm(alg.to_str().unwrap())?;

    // Build header
    let header = Header::new(algorithm);

    // Build claims
    let claims = build_claims(claims_obj)?;

    // Create encoding key
    let key = EncodingKey::from_secret(secret_str.as_bytes());

    // Generate token
    let token = encode(&header, &claims, &key)
        .map_err(|e| VmError::RuntimeError(format!("JWT encoding error: {}", e)))?;

    Ok(Value::String(state.intern(token.as_bytes())))
}

fn jwt_decode<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 3 {
        return Err(VmError::RuntimeError(
            "decode() requires token string, secret key, and algorithm".into(),
        ));
    }

    // Get token
    let token = args[0].as_string()?;
    let token_str = token.to_str().unwrap();

    // Get secret key
    let secret = args[1].as_string()?;
    let secret_str = secret.to_str().unwrap();

    // Get algorithm
    let alg = args[2].as_string()?;
    let algorithm = parse_algorithm(alg.to_str().unwrap())?;

    // Create decoding key
    let key = DecodingKey::from_secret(secret_str.as_bytes());

    // Setup validation
    let mut validation = Validation::new(algorithm);
    validation.required_spec_claims.clear(); // Don't require any specific claims

    // Decode token
    let token_data: TokenData<Claims> = decode(token_str, &key, &validation)
        .map_err(|e| VmError::RuntimeError(format!("JWT decoding error: {}", e)))?;

    // Convert claims to AIScript object
    Ok(claims_to_object(state.get_context(), token_data.claims))
}
