<div align="center">
<pre>
             _         _ 
 ___ ___ ___| |_ ___ _| |
|_ -|  _|  _| . |  _| . |
|___|___|_| |___|_| |___|
</pre>

a minimal tui sports data tracker.

written in rust with ratatui.

<a href="#installation">install</a> | <a href="https://coff.ee/chuckswung">bmac</a>
<br>

<a href="https://i.imgur.com/v2QoPb5.png" target="_blank">
    <img src="https://i.imgur.com/v2QoPb5.png" width="666">
</a>
<p align="center">
<sup><sub>screenshot also features <a href="https://gitlab.com/jallbrit/cbonsai">cbonsai</a> by john allbritten.</sub></sup>
</p>
</div>

### about

**scrbrd** is a fast, minimal, cli application for tracking live sports events in your terminal. it fetches data from espn's unofficial api and renders the information using [ratatui](https://ratatui.rs/) for a visually clean interface. 

**scrbrd** parses and presents details such as current scores, inning/quarter/period, team records, schedules, and live status — all within a compact, readable tui format.

### features

- live score display with real-time game data
- league and team filtering
- game status: period, inning, record
- auto-refresh and manual refresh support
- clean, minimal terminal interface

### built with

- [rust](https://rust-lang.org/)
- [ratatui](https://ratatui.rs/)
- [crossterm](https://github.com/crossterm-rs/crossterm)
- [serde](https://serde.rs/)
- [tokio](https://tokio.rs/)

### installation

**scrbrd** requires [rust with cargo](https://crates.io/) to run.

**using crates.io**
```bash
cargo install scrbrd
```

**from source**
```bash
cargo install --git https://github.com/chuckswung/scrbrd
```


### usage

#### commands
```
# show league scores
scrbrd -l <league>

# filter by team
scrbrd -l <league> -t <team>

# supported leagues 
mlb, nba, wnba, nfl, nhl, mls, nwsl, prem

# supported teams
all of them! you'll have to use their full name (guardians, buccaneers, etc)
club nicknames (guards, bucs, etc) are coming in a future update
```

#### controls
| Key | Action |
|:----|:-------|
| `↓` | scroll down |
| `↑` | scroll up |
| `r` | force refresh |
| `q` | quit  |

### upcoming

- [ ]  tweak ui
- [ ]  add nicknames
- [ ]  add box scores and stats

### contributing

contributions are more than welcome! i'm still a rookie dev and would love to collaborate with other developers.

to contribute:
1. fork the repo
2. create a new branch (`git checkout -b feature-name`)
3. commit your changes (`git commit -m 'add new feature'`)
4. push to the branch (`git push origin feature-name`)
5. open a pr! :D

bug reports, feature ideas, and feedback are appreciated via issues or discussions. 

### license

this project is licensed under the MIT license. see the <a href="./LICENSE">LICENSE</a> file for details.

### author

Chuck Swung - [@chuckswung](https://github.com/chuckswung)

discord: chuckswung | email: [chuckswung@gmail.com](mailto:chuckswung@gmail.com)