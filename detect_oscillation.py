#!/usr/bin/env python3
"""
Detect oscillation in watch logs by finding repeating cycles in focus Y values.
Usage: python3 detect_oscillation.py <watch_log_file>
"""

import re
import sys

def analyze_focus_oscillation(log_file):
    """Extract focus Y values and detect if they're oscillating in a cycle."""
    with open(log_file) as f:
        lines = f.readlines()

    y_values = []
    for line in lines:
        match = re.search(r'"focus":\[([^,]+),([^,]+),([^\]]+)\]', line)
        if match:
            y = float(match.group(2))
            y_values.append(round(y, 2))

    if len(y_values) < 10:
        return {"status": "insufficient_data", "values": len(y_values)}

    # Get unique transitions (remove consecutive duplicates)
    transitions = []
    for y in y_values:
        if len(transitions) == 0 or y != transitions[-1]:
            transitions.append(y)

    # Check if last 50 transitions show a repeating pattern
    last_50 = transitions[-50:] if len(transitions) >= 50 else transitions

    # Look for cycles: if we see the same sequence of 3+ values repeat
    cycle_detected = False
    cycle_length = 0
    cycle_pattern = []

    for length in range(3, 15):  # Check cycle lengths from 3 to 14
        if len(last_50) < length * 3:  # Need at least 3 repetitions
            continue

        # Extract last 'length' values as potential pattern
        pattern = last_50[-length:]

        # Check if this pattern repeats before it
        prev_pattern = last_50[-(length*2):-length]
        prev_prev_pattern = last_50[-(length*3):-(length*2)] if len(last_50) >= length*3 else None

        if pattern == prev_pattern:
            if prev_prev_pattern is None or pattern == prev_prev_pattern:
                cycle_detected = True
                cycle_length = length
                cycle_pattern = pattern
                break

    result = {
        "status": "oscillating" if cycle_detected else "converging",
        "total_values": len(y_values),
        "unique_transitions": len(transitions),
        "final_value": y_values[-1],
    }

    if cycle_detected:
        result["cycle_length"] = cycle_length
        result["cycle_pattern"] = cycle_pattern
        result["message"] = f"OSCILLATION DETECTED: Cycling through {cycle_length} values"
    else:
        # Check if it's stable (last 20 values are the same)
        if len(y_values) >= 20 and len(set(y_values[-20:])) == 1:
            result["message"] = f"CONVERGED: Stable at {y_values[-1]}"
        else:
            result["message"] = f"CONVERGING: Last value {y_values[-1]}, {len(set(last_50))} unique in last 50"

    return result

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python3 detect_oscillation.py <watch_log_file>")
        sys.exit(1)

    result = analyze_focus_oscillation(sys.argv[1])

    print(f"\n{'='*60}")
    print(f"Focus Oscillation Analysis")
    print(f"{'='*60}")
    print(f"Status: {result['status'].upper()}")
    print(f"Total updates: {result['total_values']}")
    print(f"Unique transitions: {result['unique_transitions']}")
    print(f"Final value: {result['final_value']}")

    if "cycle_pattern" in result:
        print(f"\nCycle detected ({result['cycle_length']} values):")
        for i, val in enumerate(result['cycle_pattern'], 1):
            print(f"  {i}. {val}")

    print(f"\n{result['message']}")
    print(f"{'='*60}\n")

    # Exit code: 0 = converged, 1 = oscillating
    sys.exit(0 if result['status'] == "converging" else 1)
