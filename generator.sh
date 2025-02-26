#!/bin/bash

# Configuration variables
LOG_FILE="test_logs.log"
MIN_BURST=1
MAX_BURST=1000
MIN_SLEEP=1
MAX_SLEEP=10

# Array of possible error messages
declare -a ERROR_MESSAGES=(
    "Database connection failed"
    "Null pointer exception"
    "File not found"
    "Access denied"
    "Out of memory"
    "Network timeout occurred"
    "Illegal argument provided"
    "User authentication failed"
)

# Function to generate a random IP address in the format 192.168.X.Y
generate_ip() {
    echo "192.168.$((RANDOM % 254 + 1)).$((RANDOM % 254 + 1))"
}

# Function to get current timestamp in ISO 8601 format
get_timestamp() {
    date -u +"%Y-%m-%dT%H:%M:%SZ"
}

# Function to generate a random log level
generate_level() {
    local rand=$((RANDOM % 3))
    case $rand in
        0) echo "ERROR" ;;
        1) echo "INFO" ;;
        2) echo "DEBUG" ;;
    esac
}

# Function to generate an error message for ERROR level logs
generate_error_message() {
    local level=$1
    if [ "$level" = "ERROR" ]; then
        local index=$((RANDOM % ${#ERROR_MESSAGES[@]}))
        echo "Error 500 - ${ERROR_MESSAGES[$index]}"
    else
        echo ""
    fi
}

# Function to generate a single log entry
generate_log_entry() {
    local level=$(generate_level)
    local ip=$(generate_ip)
    local timestamp=$(get_timestamp)
    local error_message=$(generate_error_message "$level")
    echo "[$timestamp] $level - IP:$ip $error_message"
}

# Create or clear the log file
#> "$LOG_FILE"

echo "Starting log generation. Press Ctrl+C to stop..."

# Main loop to generate logs
while true; do
    # Generate a random burst size
    burst_size=$((RANDOM % (MAX_BURST - MIN_BURST + 1) + MIN_BURST))
    
    # Generate burst of log entries
    for ((i=0; i<burst_size; i++)); do
        generate_log_entry #>> "$LOG_FILE"
    done
    
    # Random sleep between bursts (in milliseconds)
    sleep_time=$((RANDOM % (MAX_SLEEP - MIN_SLEEP + 1) + MIN_SLEEP))
    sleep "0.$sleep_time"
done
