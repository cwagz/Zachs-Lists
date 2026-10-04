from flask import Flask

from app.socketio import init_socketio


def test_socketio_connect_and_job_room_updates() -> None:
    application = Flask(__name__)
    application.config.update(TESTING=True, SECRET_KEY="example-test-session-key")
    server = init_socketio(application)
    client = server.test_client(application)
    try:
        assert client.is_connected()
        assert any(event["name"] == "connected" for event in client.get_received())
        client.emit("subscribe:jobs", {"user_id": "example-user"})
        server.emit(
            "job:progress",
            {"job_id": "example-job", "status": "processing"},
            to="jobs:example-user",
        )
        events = client.get_received()
        assert any(
            event["name"] == "job:progress"
            and event["args"][0]["job_id"] == "example-job"
            for event in events
        )
    finally:
        if client.is_connected():
            client.disconnect()
