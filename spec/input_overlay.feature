Feature: Input Overlay in Recordings
  Keystrokes and mouse actions are burned into the exported MP4 as a visual overlay,
  similar to KeyCastr. This lets viewers see exactly what input was sent at each moment.

  Scenario: Keystrokes appear as overlay in recording
    Given a session "demo" is running and recording
    When I run "frameterm type -s demo hello"
    And I export the recording
    Then the MP4 should show a keystroke overlay displaying "h", "e", "l", "l", "o" as they were typed

  Scenario: Key combos appear as overlay
    Given a session "demo" is running and recording
    When I run "frameterm key -s demo Ctrl+C"
    And I export the recording
    Then the MP4 should show a keystroke overlay displaying "Ctrl+C"

  Scenario: Key sequences appear in order
    Given a session "demo" is running and recording
    When I run "frameterm key -s demo 'Escape : w q Enter'"
    And I export the recording
    Then the MP4 should show overlays for "Esc", ":", "w", "q", "Enter" in sequence

  Scenario: Mouse clicks appear as overlay
    Given a session "demo" is running and recording
    When I run "frameterm click -s demo 10 5"
    And I export the recording
    Then the MP4 should show a click indicator at row 10, column 5

  Scenario: Scroll events appear as overlay
    Given a session "demo" is running and recording
    When I run "frameterm scroll -s demo down 3"
    And I export the recording
    Then the MP4 should show a scroll indicator with direction and amount

  Scenario: Overlay fades out after a short duration
    Given a session "demo" is running and recording
    When I run "frameterm type -s demo x"
    And I wait 2 seconds
    And I export the recording
    Then the keystroke overlay "x" should be visible briefly and then fade out

  Scenario: Overlapping inputs are stacked
    Given a session "demo" is running and recording
    When I run "frameterm key -s demo 'a b c' --delay 50"
    And I export the recording
    Then the overlay should show recent keystrokes stacked, with older ones fading out

  Scenario: Overlay position does not obscure terminal content
    Given a session "demo" is running and recording
    When I send various inputs
    And I export the recording
    Then the keystroke overlay should be positioned at the bottom of the frame
    And mouse click indicators should be positioned at the click coordinates

  Scenario: Disable input overlay
    Given a session "demo" is running and recording
    When I run "frameterm record export -s demo --no-overlay"
    Then the MP4 should not contain any input overlays

  Scenario: Overlay includes timestamps
    Given a session "demo" is running and recording
    When I send input at various times
    And I export the recording
    Then each overlay event should appear at the correct timestamp in the video
