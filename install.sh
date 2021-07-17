#!/bin/bash

FILE_REPOSITORY_PATH=/file_repository

LIB_PATH="/usr/lib/kp-rust"
CUR_DIR=$(dirname $(realpath $0))
OLD_DIR=$(realpath .)

# Installs the library needed to compile and builds
function build() {
    sudo apt-get install build-essential
    sudo apt-get install libdbus-1-dev
    sudo apt-get install libcec-dev
    cargo build --release
}

# Ensure the user kp that is used to run the proxy exists
function create_user() {
    id -u kp > /dev/null 2>&1
    if [[ $? -ne 0 ]]; then
        echo "Creating kp user"
        sudo useradd -M kp
    else
        echo "User kp already exists. Not recreating it"
    fi
}

# Copies the useful files to where they're needed
function install_lib() {
    local from=$1
    local to=$2
    echo "Copying files"
    sudo mkdir -p $to
    sudo cp $from/target/release/kodiproxy $to
    copy_settings_file "$from/resources/kodiproxy.json" "$to/kodiproxy.json"
    sudo chown -R kp:kp $to
}

# Gather some information from the user
function get_settings() {
    local lastInstall=$CUR_DIR/install.last
    local user_input
    if [[ -e $lastInstall ]]; then
        source $lastInstall
    fi

    PORT=${PORT:-"8079"}
    echo "Please give the port on which the server will be launched (default: $PORT):"
    read user_input
    PORT=${user_input:-"$PORT"}
    if [[ -z $PORT ]]; then
        echo "Error: no port provided"
        \exit 1
    fi

    echo -ne "Please give the IP of the receiver "
    if [[ -n $RECEIVER_IP ]]; then
        echo "(default: $RECEIVER_IP):"
    else
        echo "(eg: 192.168.2.40):"
    fi
    read user_input
    RECEIVER_IP=${user_input:-"$RECEIVER_IP"}
    if [[ -z $RECEIVER_IP ]]; then
        echo "Error: no receiver ip given"
        \exit 1
    fi

    echo -ne "Please give the url of the jsonrpc server "
    if [[ -n $JRPC_TARGET ]]; then
        echo "(default: $JRPC_TARGET):"
    else
        echo "(eg: http://localhost:8081/jsonrpc):"
    fi
    read user_input
    JRPC_TARGET=${user_input:-"$JRPC_TARGET"}
    if [[ -z $JRPC_TARGET ]]; then
        echo "Error: no jsonrpc target given"
        \exit 1
    fi

    printf "PORT=$PORT\nRECEIVER_IP=$RECEIVER_IP\nJRPC_TARGET=$JRPC_TARGET\n" > $lastInstall
}

# Copies a file where it's needed, doing some substitutions first
function copy_settings_file() {
    cat "$1" | \
            sed "s;%FILE_REPOSITORY_PATH%;$FILE_REPOSITORY_PATH;g" | \
            sed "s;%JRPC_TARGET%;$JRPC_TARGET;g" | \
            sed "s;%RECEIVER_IP%;$RECEIVER_IP;g" | \
            sed "s;%PORT%;$PORT;g" | \
            sudo tee "$2" > /dev/null
}

# Creates and starts the service so it's run in the background and started automatically
function install_systemd_service() {
    echo 'Installing systemd service'
    local service="kodiproxy.service"
    local enabled=$(systemctl is-enabled $service)
    local active='notactive'
    if [[ $enabled == 'enabled' ]]; then
        active=$(systemctl is-active $service)
    fi
    if [[ $active == 'active' ]]; then
        sudo systemctl stop $service
    fi
    if [[ $enabled == 'enabled' ]]; then
        sudo systemctl disable $service
    fi

    copy_settings_file "$CUR_DIR/resources/kodiproxy.systemd.service" "/lib/systemd/system/kodiproxy.service"

    sudo systemctl enable $service
    sudo systemctl start $service
}

function setup_file_repo {
    sudo mkdir -p $FILE_REPOSITORY_PATH
    sudo chown -R kp:kp $FILE_REPOSITORY_PATH
}

# Do all the stuff
build
create_user
setup_file_repo
get_settings
install_lib $CUR_DIR $LIB_PATH
install_systemd_service
