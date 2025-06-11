# scrbrd

a minimal terminal sports scoreboard using the ESPN API, written in rust

## installation
`scrbrd` requires [rust with cargo](https://crates.io/)

### using crates.io
`cargo install scrbrd`

### from source
`cargo install --git https://github.com/chuckswung/scrbrd`

## usage
```
# show mlb scores
scrbrd -l mlb

# filter by team
scrbrd -l mlb -t guardians

# support leagues: mlb, nba, wnba, nfl, nhl, mls, nwsl, prem
```
## features

- live game updates with real-time scores
- support for 8 major sports leagues
- clean, minimal terminal interface
- team filtering

## upcoming

- [ ] tweak ui
- [ ] add club nicknames
- [ ] add box scores and statistcs
- [ ] fix innings parsing (missing end)

## contributing

i'm still a rookie so i'm not really looking for contributions, but feel free to fork the repo and make your own version of `scrbrd`!