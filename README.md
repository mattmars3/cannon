# File Cannon
Shoot off files for others to download. Quite primitive, but powerful. Like a big ol' cannon.

*Note:* this is still in active development. Beware when deploying this on public-facing servers.

## Installation
*Install Dependencies*
Must have `Cargo`/`Rust` installed.

## Setup
### Client
The client allows you to pass in a specific server and api key as command line arguments. However, this may violate the convenience-first philosophy
of the tool. Instead, you should set the `CANNON_SERVER` and `CANNON_API_KEY` environment variables.

### Server
To run the server, you **MUST** specify the `CANNON_API_KEY` environment variable. Currently, this can be set to any string, though
it is recommended to use 

## TODO:
- [ ] Confirm it compiles on OpenBSD
- [ ] Improve error handling and user interface
- [ ] Allow arbitrary file upload via piping (generates text file)
- [ ] Improve documentation and write tests
- [ ] File expiration
- [ ] Systemd, openrc, and shell startup scripts
- [ ] Constant-time password comparison with `subtle`
- [ ] Password files instead of plaintext stored in environment
