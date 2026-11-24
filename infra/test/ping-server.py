"""A stand-in for Healthchecks.io: answers 200 to every GET and logs "GET <path>"."""
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

LOG = sys.argv[1]


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        with open(LOG, "a", encoding="utf-8") as log:
            log.write(f"GET {self.path}\n")
        self.send_response(200)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"OK")

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", 8099), Handler).serve_forever()
