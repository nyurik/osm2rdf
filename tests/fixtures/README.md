Most of these files were copied from [libosmium](https://github.com/osmcode/libosmium/tree/master/test) project's test
director using `copy.py` script that renames them and puts them in the `libomium` dir.

A `just gen-pbf` command will then run osmium tool to generate a pbf file from the osm file in all dirs.

```shell
python3 copy.py "...libosmium...dir...path..."
just gen-pbf
```

## LICENSE

The libosmium test files are licensed under BOOST Software License. See LICENSE file for details.
