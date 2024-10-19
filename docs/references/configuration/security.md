
```toml
[security.jwt]
secret = "secret"
expiration = 3600
header = "Authorization"

[security.csrf]
enabled = true
header = "X-CSRF-Token"

[security.ssl]
enabled = false
cert = "cert.pem"
key = "key.pem"
```

## JSON Web Token

## CSRF

## SSL