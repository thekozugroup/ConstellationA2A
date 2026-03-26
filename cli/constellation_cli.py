#!/usr/bin/env python3
"""
Constellation CLI - Management tool for Constellation A2A

Usage:
    python cli/constellation_cli.py status
    python cli/constellation_cli.py send '#constellation:constellation.local' 'Hello!'
    python cli/constellation_cli.py list-rooms
    python cli/constellation_cli.py list-agents
    python cli/constellation_cli.py register-agent myagent
"""
import argparse
import hashlib
import hmac
import json
import os
import sys
import urllib.request
import urllib.error
import urllib.parse

DEFAULT_HOMESERVER = os.environ.get("MATRIX_HOMESERVER", "http://localhost:8448")
DEFAULT_USERNAME = os.environ.get("AGENT_USERNAME", "admin")
DEFAULT_PASSWORD = os.environ.get("AGENT_PASSWORD", "admin-secret")


def api_request(homeserver, method, endpoint, data=None, token=None):
    """Make a Matrix API request."""
    url = f"{homeserver}{endpoint}"
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"

    req_data = json.dumps(data).encode() if data else None
    req = urllib.request.Request(url, data=req_data, headers=headers, method=method)

    try:
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        try:
            return json.loads(body)
        except json.JSONDecodeError:
            return {"error": body, "status": e.code}
    except urllib.error.URLError as e:
        return {"error": str(e.reason), "status": 0}


def login(homeserver, username, password):
    """Login and return access token."""
    result = api_request(homeserver, "POST", "/_matrix/client/v3/login", {
        "type": "m.login.password",
        "identifier": {"type": "m.id.user", "user": username},
        "password": password,
    })
    return result.get("access_token")


def cmd_status(args):
    """Check server status."""
    print(f"Checking {args.homeserver}...")

    versions = api_request(args.homeserver, "GET", "/_matrix/client/versions")
    if "error" in versions:
        print(f"  OFFLINE - {versions['error']}")
        return 1

    print(f"  Server: ONLINE")
    print(f"  Versions: {', '.join(versions.get('versions', []))}")

    # Try server version
    server_info = api_request(args.homeserver, "GET", "/_matrix/federation/v1/version")
    if "server" in server_info:
        s = server_info["server"]
        print(f"  Software: {s.get('name', 'unknown')} {s.get('version', '')}")

    return 0


def cmd_send(args):
    """Send a message to a room."""
    token = login(args.homeserver, args.username, args.password)
    if not token:
        print("Login failed")
        return 1

    import time
    txn_id = str(int(time.time() * 1000))
    room_id = args.room

    # Resolve room alias if needed
    if room_id.startswith("#"):
        encoded = urllib.parse.quote(room_id)
        result = api_request(args.homeserver, "GET",
                           f"/_matrix/client/v3/directory/room/{encoded}", token=token)
        if "room_id" in result:
            room_id = result["room_id"]
        else:
            print(f"Could not resolve room alias: {args.room}")
            return 1

    encoded_room = urllib.parse.quote(room_id)
    result = api_request(args.homeserver, "PUT",
                        f"/_matrix/client/v3/rooms/{encoded_room}/send/m.room.message/{txn_id}",
                        {"msgtype": "m.text", "body": args.message}, token=token)

    if "event_id" in result:
        print(f"Sent: {result['event_id']}")
        return 0
    else:
        print(f"Failed: {result}")
        return 1


def cmd_list_rooms(args):
    """List rooms on the server."""
    token = login(args.homeserver, args.username, args.password)
    if not token:
        print("Login failed")
        return 1

    result = api_request(args.homeserver, "GET",
                        "/_matrix/client/v3/joined_rooms", token=token)
    rooms = result.get("joined_rooms", [])

    if not rooms:
        print("No joined rooms")
        return 0

    print(f"Joined rooms ({len(rooms)}):")
    for room_id in rooms:
        print(f"  {room_id}")
    return 0


def cmd_list_agents(args):
    """List registered agent users (requires admin)."""
    token = login(args.homeserver, args.username, args.password)
    if not token:
        print("Login failed")
        return 1

    # Use Synapse-compatible admin API (Conduit supports subset)
    result = api_request(args.homeserver, "GET",
                        "/_matrix/client/v3/joined_rooms", token=token)

    # Get members of the constellation room
    rooms = result.get("joined_rooms", [])
    if not rooms:
        print("No rooms found. Register agents and create rooms first.")
        return 0

    all_members = set()
    for room_id in rooms:
        encoded = urllib.parse.quote(room_id)
        members = api_request(args.homeserver, "GET",
                            f"/_matrix/client/v3/rooms/{encoded}/members", token=token)
        for event in members.get("chunk", []):
            if event.get("content", {}).get("membership") == "join":
                all_members.add(event["state_key"])

    print(f"Registered agents ({len(all_members)}):")
    for member in sorted(all_members):
        print(f"  {member}")
    return 0


def cmd_register_agent(args):
    """Register a new agent account."""
    # Read shared secret
    secret = os.environ.get("REGISTRATION_SECRET", "")
    if not secret and os.path.exists(".registration_secret"):
        with open(".registration_secret") as f:
            secret = f.read().strip()

    if not secret:
        print("No registration secret found. Set REGISTRATION_SECRET env var or create .registration_secret file")
        return 1

    # Get nonce
    result = api_request(args.homeserver, "GET", "/_matrix/client/v3/register")
    nonce = result.get("nonce", "")
    if not nonce:
        # Try alternative endpoint
        result = api_request(args.homeserver, "GET",
                           "/_synapse/admin/v1/register")
        nonce = result.get("nonce", "")

    if not nonce:
        print("Could not get registration nonce")
        return 1

    username = args.agent_name
    password = args.agent_password or f"{username}-constellation-secret"

    # Compute HMAC
    mac_msg = f"{nonce}\x00{username}\x00{password}\x00notadmin"
    mac = hmac.new(secret.encode(), mac_msg.encode(), hashlib.sha1).hexdigest()

    result = api_request(args.homeserver, "POST", "/_synapse/admin/v1/register", {
        "nonce": nonce,
        "username": username,
        "password": password,
        "mac": mac,
        "admin": False,
    })

    if "user_id" in result:
        print(f"Registered: {result['user_id']}")
        print(f"Password: {password}")
        return 0
    else:
        print(f"Registration failed: {result.get('error', result)}")
        return 1


def main():
    parser = argparse.ArgumentParser(
        prog="constellation",
        description="Constellation A2A - Agent-to-Agent Matrix Server CLI",
    )
    parser.add_argument("--homeserver", default=DEFAULT_HOMESERVER,
                       help=f"Matrix homeserver URL (default: {DEFAULT_HOMESERVER})")
    parser.add_argument("--username", default=DEFAULT_USERNAME,
                       help="Username for authentication")
    parser.add_argument("--password", default=DEFAULT_PASSWORD,
                       help="Password for authentication")

    subparsers = parser.add_subparsers(dest="command", required=True)

    # status
    subparsers.add_parser("status", help="Check server status")

    # send
    send_parser = subparsers.add_parser("send", help="Send a message to a room")
    send_parser.add_argument("room", help="Room ID or alias (e.g., #constellation:constellation.local)")
    send_parser.add_argument("message", help="Message text to send")

    # list-rooms
    subparsers.add_parser("list-rooms", help="List joined rooms")

    # list-agents
    subparsers.add_parser("list-agents", help="List registered agents")

    # register-agent
    reg_parser = subparsers.add_parser("register-agent", help="Register a new agent")
    reg_parser.add_argument("agent_name", help="Agent username")
    reg_parser.add_argument("--agent-password", help="Agent password (auto-generated if omitted)")

    args = parser.parse_args()

    commands = {
        "status": cmd_status,
        "send": cmd_send,
        "list-rooms": cmd_list_rooms,
        "list-agents": cmd_list_agents,
        "register-agent": cmd_register_agent,
    }

    sys.exit(commands[args.command](args))


if __name__ == "__main__":
    main()
