## Ambassador Rate Limiter in Rust

This is a rate limiter service written in Rust for [Datawires][0] [Ambassador][1],
a Cloud Native proxy/API Gateway.

### Workings

This is based on the Leaky bucket algorithm and currently supports two plans
with different rate limits. The rate limits themselves as well as the number of
plans can be tweaked for your own requirements.

### Required Headers

To identify requests it expects two headers passed on from the authentication service
that ambassador forwards to our rate-limiter service as `RateLimitDescriptor` Entries.

Your HTTP header names can be anything you want but when defining the Mapping
CRD of ambassador for your service make sure the labels are the same as:

* `xapiheader`: an API key used to uniquely identify an API user.
* `xuserheader`: the "plan" the user of the API is currently on which defines the 
    extent of the rate limiting. This should be determined by your auth service
    and added as an header to the request.

Note:
* `remote_address`: Remote IP address is used in-case your users need to use the
    API from a client device.  In case you don't need this feature you change to
    ratelimit on the bases of the `api_key` instead of the `remote_ip`.

For more details on how to setup the rate-limiting service see ambassador [docs][3]

For Example:
```yml
apiVersion: ambassador/v1
kind:  Mapping
name: {{ template "myservice.fullname" . }}_mapping
service: {{ template "myservice.fullname" . }}:{{ .Values.service.port }}
labels:
  ambassador:
    - remote_address
    - xapiheader:
        header: "x-api-key"
        omit_if_not_present: true
    - xuserheader:
        header: "x-user-plan"
        omit_if_not_present: true
```

### Authentication Service setup

In order for Ambassador to pass on the required headers from the HTTP request to
the `RateLimitService` make sure you whitelist the headers in the `AuthService`
service defined your "getambassador.io/config" annotation when deploying the
core Ambassador service along with other ambassador specific services you might have.
For more details see the ambassador [docs][4] on authentication.

For Example:
```yml
getambassador.io/config: |
  apiVersion: ambassador/v1
  kind:  AuthService
  name:  authentication
  auth_service: "washed-sheep-ambassador-auth-service:3001"
  path_prefix: "/extauth"
  allowed_request_headers:
  - "x-api-key"
  - "x-api-secret"
  allowed_authorization_headers:
  - "x-api-key"
  - "x-user-plan"
  ---
```

### Helm chart

This repo contains a [helm](helm.sh/) chart in the `helm` directory to help
deploy the service to your Kubernetes cluster.

### Logging

This project uses the [`env_logger`][2] rust crate to control logging to the stdout.

You can specify the log level with the Environment variable 
`RUST_LOG` such as `RUST_LOG=ambassador_rust_rate_limiter=debug`.

[0]: https://www.datawire.io/
[1]: https://www.getambassador.io/
[2]: https://crates.io/crates/env_logger
[3]: https://www.getambassador.io/user-guide/rate-limiting-tutorial/
[4]: https://www.getambassador.io/user-guide/auth-tutorial/
