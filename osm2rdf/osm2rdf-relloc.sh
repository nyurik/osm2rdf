#!/usr/bin/env bash
set -e
echo "########### Running updateRelLoc ###########"

NODES_CACHE="${OSM_RDF_DATA}/nodes.cache"
[[ -n "${IS_FULL_PLANET}" ]] && CACHE_STRATEGY="dense" || CACHE_STRATEGY="sparse"

if [[ ! -f "${FLAG_TTL_IMPORTED}" ]]; then
  echo "########### ERROR: osm2rdf has not been imported"
  exit 1
elif [[ ! -f "${NODES_CACHE}" ]]; then
  echo "########### ERROR: osm2rdf node cache not found:  ${NODES_CACHE}"
  exit 1
fi


set -x
python3 updateRelLoc.py                  \
    --nodes-file "${NODES_CACHE}"        \
    --cache-strategy "${CACHE_STRATEGY}" \
    --host "${BLAZEGRAPH_URL}"
