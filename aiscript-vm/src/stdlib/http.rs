use ahash::AHasher;
use std::{collections::HashMap, hash::BuildHasherDefault};

use aiscript_arena::{Gc, RefLock};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio::runtime::Handle;

use crate::{
    NativeFn, Value,
    module::ModuleKind,
    object::Object,
    string::InternedString,
    vm::{Context, VmError},
};

pub fn create_http_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.http");

    let exports = [
        ("get", Value::NativeFunction(NativeFn(http_get))),
        ("post", Value::NativeFunction(NativeFn(http_post))),
        ("put", Value::NativeFunction(NativeFn(http_put))),
        ("delete", Value::NativeFunction(NativeFn(http_delete))),
        ("patch", Value::NativeFunction(NativeFn(http_patch))),
        ("head", Value::NativeFunction(NativeFn(http_head))),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();

    ModuleKind::Native { name, exports }
}

fn parse_headers(
    headers: HashMap<InternedString, Value, BuildHasherDefault<AHasher>>,
) -> HeaderMap {
    let mut header_map = HeaderMap::new();
    for (key, value) in headers {
        if let Value::String(s) = value {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_bytes(s.as_bytes()),
            ) {
                header_map.insert(name, val);
            }
        }
    }
    header_map
}

async fn response_to_object(
    ctx: Context<'_>,
    response: reqwest::Response,
) -> Result<Value<'_>, VmError> {
    let mut resp_obj = Object::default();

    let status = response.status();
    resp_obj
        .fields
        .insert(ctx.intern(b"status"), Value::Number(status.as_u16() as f64));

    resp_obj.fields.insert(
        ctx.intern(b"statusText"),
        Value::String(ctx.intern(status.canonical_reason().unwrap_or("").as_bytes())),
    );

    resp_obj
        .fields
        .insert(ctx.intern(b"ok"), Value::Boolean(status.is_success()));

    let mut headers_obj = Object::default();
    for (name, value) in response.headers() {
        headers_obj.fields.insert(
            ctx.intern(name.as_str().as_bytes()),
            Value::String(ctx.intern(value.to_str().unwrap_or("").as_bytes())),
        );
    }
    resp_obj.fields.insert(
        ctx.intern(b"headers"),
        Value::Object(Gc::new(&ctx, RefLock::new(headers_obj))),
    );

    let headers = response.headers().clone();
    // Add response text
    let text = response
        .text()
        .await
        .map_err(|e| VmError::RuntimeError(format!("Failed to read response body: {}", e)))?;

    resp_obj.fields.insert(
        ctx.intern(b"text"),
        Value::String(ctx.intern(text.as_bytes())),
    );

    // Try to parse JSON if content-type is application/json
    if let Some(content_type) = headers.get("content-type") {
        if content_type
            .to_str()
            .unwrap_or("")
            .contains("application/json")
        {
            if let Ok(json) = serde_json::from_str(&text) {
                resp_obj
                    .fields
                    .insert(ctx.intern(b"json"), Value::from_serde_value(ctx, &json));
            }
        }
    }

    Ok(Value::Object(Gc::new(&ctx, RefLock::new(resp_obj))))
}

async fn make_request(
    method: reqwest::Method,
    url: &str,
    headers: HeaderMap,
    body: Option<String>,
) -> Result<reqwest::Response, reqwest::Error> {
    let client = reqwest::Client::new();
    let mut request = client.request(method, url);
    request = request.headers(headers);

    if let Some(body) = body {
        request = request.body(body);
    }

    request.send().await
}

fn http_get<'gc>(
    ctx: &mut crate::vm::State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError("URL required for GET request".into()));
    }

    let url = args[0].as_string()?.to_string();
    let headers_map = if args.len() > 1 {
        if let Value::Object(obj) = args[1] {
            let borrowed = obj.borrow();
            parse_headers(borrowed.fields.clone())
        } else {
            return Err(VmError::RuntimeError("Headers must be an object".into()));
        }
    } else {
        HeaderMap::new()
    };

    // Execute request and process response in runtime
    let result = Handle::current().block_on(async {
        let response = make_request(reqwest::Method::GET, &url, headers_map, None)
            .await
            .map_err(|e| VmError::RuntimeError(format!("HTTP request failed: {}", e)))?;

        response_to_object(ctx.get_context(), response).await
    })?;

    Ok(result)
}

fn http_post<'gc>(
    ctx: &mut crate::vm::State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() < 2 {
        return Err(VmError::RuntimeError(
            "URL and body required for POST request".into(),
        ));
    }

    let url = args[0].as_string()?.to_string();
    let body = args[1].as_string()?.to_string();
    let headers_map = if args.len() > 2 {
        if let Value::Object(obj) = args[2] {
            let borrowed = obj.borrow();
            parse_headers(borrowed.fields.clone())
        } else {
            return Err(VmError::RuntimeError("Headers must be an object".into()));
        }
    } else {
        HeaderMap::new()
    };

    let result = Handle::current().block_on(async {
        let response = make_request(reqwest::Method::POST, &url, headers_map, Some(body))
            .await
            .map_err(|e| VmError::RuntimeError(format!("HTTP request failed: {}", e)))?;

        response_to_object(ctx.get_context(), response).await
    })?;

    Ok(result)
}

fn http_put<'gc>(
    ctx: &mut crate::vm::State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() < 2 {
        return Err(VmError::RuntimeError(
            "URL and body required for PUT request".into(),
        ));
    }

    let url = args[0].as_string()?.to_string();
    let body = args[1].as_string()?.to_string();
    let headers = if args.len() > 2 {
        if let Value::Object(obj) = args[2] {
            obj.borrow().fields.clone()
        } else {
            return Err(VmError::RuntimeError("Headers must be an object".into()));
        }
    } else {
        HashMap::default()
    };

    let headers_map = parse_headers(headers);

    let result = Handle::current().block_on(async {
        let response = make_request(reqwest::Method::PUT, &url, headers_map, Some(body))
            .await
            .map_err(|e| VmError::RuntimeError(format!("HTTP request failed: {}", e)))?;

        response_to_object(ctx.get_context(), response).await
    })?;

    Ok(result)
}

fn http_delete<'gc>(
    ctx: &mut crate::vm::State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "URL required for DELETE request".into(),
        ));
    }

    let url = args[0].as_string()?.to_string();
    let headers_map = if args.len() > 1 {
        if let Value::Object(obj) = args[1] {
            let borrowed = obj.borrow();
            parse_headers(borrowed.fields.clone())
        } else {
            return Err(VmError::RuntimeError("Headers must be an object".into()));
        }
    } else {
        HeaderMap::new()
    };

    let result = Handle::current().block_on(async {
        let response = make_request(reqwest::Method::DELETE, &url, headers_map, None)
            .await
            .map_err(|e| VmError::RuntimeError(format!("HTTP request failed: {}", e)))?;

        response_to_object(ctx.get_context(), response).await
    })?;

    Ok(result)
}

fn http_patch<'gc>(
    ctx: &mut crate::vm::State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() < 2 {
        return Err(VmError::RuntimeError(
            "URL and body required for PATCH request".into(),
        ));
    }

    let url = args[0].as_string()?.to_string();
    let body = args[1].as_string()?.to_string();
    let headers = if args.len() > 2 {
        if let Value::Object(obj) = args[2] {
            obj.borrow().fields.clone()
        } else {
            return Err(VmError::RuntimeError("Headers must be an object".into()));
        }
    } else {
        HashMap::default()
    };

    let headers_map = parse_headers(headers);

    let result = Handle::current().block_on(async {
        let response = make_request(reqwest::Method::PATCH, &url, headers_map, Some(body))
            .await
            .map_err(|e| VmError::RuntimeError(format!("HTTP request failed: {}", e)))?;

        response_to_object(ctx.get_context(), response).await
    })?;

    Ok(result)
}

fn http_head<'gc>(
    ctx: &mut crate::vm::State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "URL required for HEAD request".into(),
        ));
    }

    let url = args[0].as_string()?.to_string();
    let headers = if args.len() > 1 {
        if let Value::Object(obj) = args[1] {
            obj.borrow().fields.clone()
        } else {
            return Err(VmError::RuntimeError("Headers must be an object".into()));
        }
    } else {
        HashMap::default()
    };

    let headers_map = parse_headers(headers);

    let result = Handle::current().block_on(async {
        let response = make_request(reqwest::Method::HEAD, &url, headers_map, None)
            .await
            .map_err(|e| VmError::RuntimeError(format!("HTTP request failed: {}", e)))?;

        response_to_object(ctx.get_context(), response).await
    })?;

    Ok(result)
}
