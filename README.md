# bb - CLI-based bookmark manager for nerds


## Overview

**bb** is a CLI based bookmark manager designed for people who like to collect shit only to never revisit it again. It supports image previews and comes with a  simple webui. **bb** can be ran as a standalone CLI utility or deployed as a daemon on a remote server. Additionally, **bb** scrapes the web pages for you in order to retrieve metadata. 

![](https://github.com/bbonvi/bb/blob/main/screenshots/shot1.png?raw=true)
![](https://github.com/bbonvi/bb/blob/main/screenshots/shot2.png?raw=true)

**This project is heavily work-in-progress!**

***bb** is inspired by [buku](https://github.com/jarun/buku).*


## Features

- **Tags**: Easily categorize your bookmarks with tags for better organization.

- **Rules**: Create custom rules using YAML configuration. Define matching queries for titles, URLs, or descriptions, and apply actions based on those matches. For example, bb can automatically assign tag "dev" for every url containing "github.com".

- **Scrape Metadata**: When you create a bookmark, bb attempts to fetch metadata from the page via a simple GET request. It extracts the title, description, and URL for page thumbnails (og:image metadata). If the request fails, bb will launch a headless chromium instance to retrieve the same information and take a screenshot of the page as well as favicon. Additionally, the chrome instance will attempt to bypass captchas.

- **Web UI**: Manage your bookmarks through a user-friendly web interface. This feature is particularly useful as bb stores screenshots and favicons of your pages for quick reference.

- **Standalone CLI Tool or Daemon**: Run bb as a standalone CLI tool or deploy it as a daemon on a remote server. Use the bb-cli as a lightweight client to connect to the server over HTTP.

## Installation

*There are no precompiled binaries for now.*

To install bb, follow these steps:

1. Ensure you have Rust installed on your machine. If not, you can install it from [rust-lang.org](https://www.rust-lang.org/).

2. Clone the repository:

   ```bash
   git clone https://github.com/bbonvi/bb.git
   cd bb
   ```

3. Build the project:

   ```bash
   cargo build --release
   # move binary to your PATH
   sudo mv ./target/release/bb /usr/local/bin/bb
   ```

4. Build web-ui (optional):

   ```bash
   cd client
   yarn
   yarn run build --release
   cd ../
   ```
   
## Usage

### Standalone CLI:
   
   ```bash
    # this will create bookmark and attempt to fetch metadata
    bb add --url "https://github.com/bbonvi/bb"

    # This will open up a neovim window where you can fill up the details
    EDITOR=nvim bb add --editor

    # output all bookmarks
    bb search
   ```

### Daemon:

   ```bash
    # start the daemon
    RUST_LOG=info bb daemon

    # this will connect to bb daemon at localhost:8080 and create a bookmark.
    # --async-meta parameter makes it so daemon fetches metadata in background
    # and you immediately get a response back, without a wait.
    BB_ADDR=http://localhost:8080 bb add --async-meta --url "https://github.com/bbonvi/bb"

    # after daemon completes the fetch, you can query bookmark by its title
    BB_ADDR=http://localhost:8080 bb search --title bb
   ```

### Running daemon in docker
```bash

# build with headless chrome
docker build -t bb:latest -f daemon.Dockerfile .
# build without headless chrome
docker build --build-arg NO_HEADLESS=true -t bb:latest -f daemon.Dockerfile .

docker volume create bb-data

# run and open http://localhost:8080
docker run --rm -it -v bb-data:/root/.local/share/bb -p 8080:8080 bb:latest

```

### WebUI

When running bb as daemon, you can access webui at [http://localhost:8080/](http://localhost:8080/) 


# API

### CLI API references
[API.md](https://github.com/bbonvi/bb/blob/main/API.md)

### Environment Variables

| Variable      | Description      | Default      | Example |
| ------------- | ---------------- | ------------ | ------- |
| `RUST_LOG`            | log level | error | warn |
| `BB_BASE_PATH`        | Base path for bookmarks, configs and thumbnails       | `~/.local/share/bb`   | `~/.local/share/bb`     |
| `BB_ADDR`             | Daemon http address                                   |                       | `http://localhost:8080` |
| `BB_BASIC_AUTH`       | Optional basic auth for daemon authorization.         |                       | `myusername:mypassword` |
| `HTTP_PROXY`          | Proxy for all meta requests                           |                       | `socks5://127.0.0.1:8060` |
| `OPT_PROXY`           | An optional proxy that will be used in case default (no proxy/HTTP_PROXY) request fails. Useful if bb needs to access region locked website, but you don't want to increase the probability of captcha. | | `socks5://127.0.0.1:8060` |
| `CHROME_PATH`         | A path to chromium binary                             | `chromium`            | `/usr/sbin/chromium`    |
| `EDITOR`              | Your default text editor                              | `vim`                 | `nvim`                  |
| `SHELL`               | Shell to launch editor with                           | `/usr/sbin/bash`      | `/bin/bash`             |


