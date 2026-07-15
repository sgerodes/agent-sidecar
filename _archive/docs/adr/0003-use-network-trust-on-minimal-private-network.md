# Use Network Trust on a Minimal Private Network

The v1 backend-to-sidecar API relies on Docker network membership rather than mTLS or bearer-token authentication. Because network trust is the authentication boundary, only the app backend, its AI Sidecar, and required database endpoint should share the private network.
