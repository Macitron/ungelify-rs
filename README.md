# ungelify-rs

This is a CLI tool for inspecting, unpacking, and repacking Mages archive file formats. It is a Rust port of the
original tool of the same name which was part of the now-abandoned
[SciAdv.Net](https://github.com/CommitteeOfZero/SciAdv.Net/tree/master/src/Tools/Ungelify) project.

## Usage

Run `./ungelify` with no arguments (or `./ungelify -h`) to display a helpful message on the command's syntax. You can
also run `./ungelify help <subcommand>` to display the subcommand's usage as well.

### List

*aliases: `ls`, `l`*

List out the file entries in the given archive. Includes each entry's ID, name, uncompressed file size, and hex offset
within the archive. Compressed entries have their size suffixed with an asterisk (`*`).

```shell
$ ./ungelify ls script.mpk
ID    Name                 Size         Offset
0     _ATCH.SCX            105.5 kiB    0xc000
1     _MAIL.SCX            218.8 kiB    0x26800
2     _STARTUP_WIN.SCX     25.8 kiB     0x5d800
...

# With compressed entries
$ ./ungelify list chara.mpk
ID    Name                 Size         Offset
0     ARI_ALA.png          2.0 MiB      0x4e800
1     ARI_ALA_.lay         112.3 kiB*   0x25b000
2     ARI_ALB.png          2.0 MiB      0x25e000
3     ARI_ALB_.lay         110.4 kiB*   0x462000
...
```

### Extract

*aliases: `ex`, `x`*

Extract the specified entries by name or ID, or all entries if none are specified. By default, entries will be
extracted to a new directory with the archive's filename minus the extension. If the filename does not have an
extension, a directory is created with the name `<archive_filename>.d`.

You can optionally supply the `-o | --output-dir <DIRECTORY>` flag to extract entries to `DIRECTORY` instead.

Glob matching is supported for specifying which entries to extract.

```shell
$ ./ungelify extract script.mpk
$ ls script
ANIME.SCX        SG02_08.SCX      SG04_03.SCX      SG05_06.SCX
CLRFLG.SCX       SG02_09.SCX      SG04_04.SCX      SG05_07.SCX
MACROSYS.SCX     SG02_10.SCX      SG04_05.SCX      SG05_08.SCX
...

$ ./ungelify x script.mpk -o ./extracted-entries SG04_17.SCX SG06_02.SCX
$ ls extracted-entries
SG04_17.SCX SG06_02.SCX

# Globs have to be quoted, else the shell will expand them into files that likely don't exist and fail
$ ./ungelify ex chara.mpk 'KUN_A*.png'
$ ls chara
KUN_ALA.png KUN_AMA.png KUN_ASA.png KUN_AXA.png
KUN_ALB.png KUN_AMB.png KUN_ASB.png KUN_AXB.png
KUN_ALC.png KUN_AMC.png KUN_ASC.png KUN_AXC.png
KUN_ALD.png KUN_AMD.png KUN_ASD.png KUN_AXD.png
```

### Replace

*aliases: `re`, `r`*

Rebuild the archive, replacing entries with the contents of the given files. Each replacement file's name must
correspond to an existing entry in the archive, else the command will fail.

```shell
$ ./ungelify r script.mpk ./replacements/SG04_05.SCX ./replacements/SG05_08.SCX

# Currently only works on *nix systems via shell globbing (and maybe Powershell, I wouldn't know),
# glob strings aren't supported yet
$ ./ungelify replace script.mpk ./replacements/*.SCX
```

## Supported File Formats

The only archive formats that are supported at this time are MAGES. archives v1 and v2, including support for compressed
entries. Further archive format support is under active development.
