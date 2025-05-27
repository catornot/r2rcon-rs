# r2rcon-rs
rcon spcec implementation in rust for r2northstar

should work

## setup

rcon will pull configs from commandline args.

| **command line arg** | **value**    |
| :------------------: | :----------: |
| `-rcon_ip_port`       | ip:port      |
| `-rcon_password`      | ascii string |

**Example:**
```
NorthstarLauncher.exe -dedicated -multiple -rcon_ip_port 127.0.0.1:27015 -rcon_password changeme
```

if any of these are missed the plugin won't work :p

after it works just connect with a rcon client

good luck!
