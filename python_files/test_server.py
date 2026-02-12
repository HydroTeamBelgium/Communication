import socket

# from http.server import BaseHTTPRequestHandler, HTTPServer

# class Handler(BaseHTTPRequestHandler):
#     def do_POST(self):
#         length = int(self.headers['Content-Length'])
#         print(self.rfile.read(length))
#         self.send_response(200)
#         self.end_headers()

# class Handler(BaseHTTPRequestHandler):
#     def do_GET(self):
#         length = int(self.headers['Content-Length'])
#         print(self.rfile.read(length))
#         self.send_response(200)
#         self.end_headers()

# HTTPServer(('0.0.0.0', 8000), Handler).serve_forever()
HOST = "192.168.24.70"
port = 5000

server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server_socket.bind((HOST, port))
server_socket.listen(socket.SOMAXCONN)
print(f"POP3 server listening on {HOST}: {port}\n")

while True:
    client_socket, addr = server_socket.accept()
    print(f"accepted connection {client_socket} addr: {addr}")