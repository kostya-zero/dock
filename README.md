# Dock

Dock is a fast FTP server that allows you to host your files and access them through any FTP-compatible client.
It also allows you to manage users and their permissions.

This server implements following commands:
- Authentication: `USER`, `PASS`
- Listing: `LIST`, `NLST`, `MLST`, `MLSD`
- Directories: `CWD`, `PWD`, `XPWD`, `SIZE`
- Download and send: `RETR`, `REST`, `STOR`
- Connection: `PORT`, `PASV`
- System: `SYST`, `TYPE`, `FEAT`, `OPTS`
