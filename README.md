# `cvardump`

A small utility for exporting a list of all cvars from a Source engine game into a spreadsheet in CSV format.

Can both connect using RCON and extract automatically, or through a manual dump from the `cvarlist` command.

## Usage

```shell
cvardump rcon --output=cvars.csv 192.168.10.100:27015 password

cvardump manual --input=cvarlist.txt --output=cvars.csv
```