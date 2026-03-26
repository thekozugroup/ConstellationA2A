FROM matrixconduit/matrix-conduit:latest

# Copy Conduit configuration
COPY conduit.toml /etc/conduit/conduit.toml

# Set environment variable to point to config
ENV CONDUIT_CONFIG="/etc/conduit/conduit.toml"

# Data persistence volume
VOLUME ["/var/lib/conduit"]

# Expose the Conduit port
EXPOSE 6167

# The base image already sets the entrypoint to the conduit binary
