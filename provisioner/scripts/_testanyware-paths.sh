#!/bin/bash
# Shared path helpers for testanyware scripts. Source this file to get:
#
#   _testanyware_state_dir  Runtime state: running-VM spec + metadata
#                         ${XDG_STATE_HOME:-$HOME/.local/state}/testanyware
#   _testanyware_data_dir   Persistent data: QEMU clones, golden images
#                         ${XDG_DATA_HOME:-$HOME/.local/share}/testanyware

_testanyware_state_dir() {
    printf '%s\n' "${XDG_STATE_HOME:-$HOME/.local/state}/testanyware"
}

_testanyware_data_dir() {
    printf '%s\n' "${XDG_DATA_HOME:-$HOME/.local/share}/testanyware"
}
