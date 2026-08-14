# Apple Container helpers

These scripts wrap the [Apple `container` CLI](https://github.com/apple/container) for Darial on Apple silicon (macOS 26+).

Full install and usage: **[docs/containers.md](../../docs/containers.md)**.

```bash
# from repository root
cp deploy/compose/.env.example deploy/compose/.env
./deploy/apple-container/build.sh
./deploy/apple-container/run-gateway.sh
# optional HTTPS terminator:
./deploy/apple-container/run-caddy.sh
./deploy/apple-container/stop.sh
```

The gateway is published only on `127.0.0.1:8443`. Ask IT to allowlist a real hostname on port 443 when you put Caddy in front on a public name.
