Most of these files were copied from [libosmium](https://github.com/osmcode/libosmium/tree/master/test) project's test
director, and encoded as `.osm.pbf` using `copy.py` script that runs osmium internally.
Make sure osmium-tools are installed.

```shell
python3 copy.py "...libosmium...dir...path..."
# internally runs    osmium cat -f pbf -o "file.osm.pbf" "file.osm"
```

## LICENSE

The libosmium test files are licensed under BOOST Software License. See LICENSE file for details.
