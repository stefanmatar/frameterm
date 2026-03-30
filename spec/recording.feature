Feature: Video Recording
  Every session is recorded and can be exported as an MP4.
  By default, exports include an input overlay (see input_overlay.feature).

  Scenario: Export recording when session ends
    Given a session "demo" has been running with activity
    When the session "demo" ends
    Then an MP4 file should be produced for session "demo"
    And the MP4 should include input overlays by default

  Scenario: Export recording on demand
    Given a session "demo" is running
    When I run "frameterm record export -s demo"
    Then an MP4 file should be produced for the session so far
    And the session should continue running

  Scenario: Export all session recordings
    Given a session "app1" has been running with activity
    And a session "app2" has been running with activity
    When I run "frameterm record export --all"
    Then MP4 files should be produced for both "app1" and "app2"

  Scenario: Export default session recording
    Given a session "default" has been running with activity
    When I run "frameterm record export"
    Then an MP4 file should be produced for the default session

  Scenario: Configurable frame rate
    Given a session "demo" has been running with activity at "--fps 30"
    When I export the recording
    Then the exported MP4 should have 30 frames per second

  Scenario: Default frame rate
    Given a session "demo" has been running with activity
    When I export the recording
    Then the exported MP4 should have 10 frames per second

  Scenario: Configurable output directory
    When I run "frameterm record export -s demo --output /tmp/recordings"
    Then the MP4 file should be written to "/tmp/recordings/"

  Scenario: Default output directory
    When I run "frameterm record export -s demo"
    Then the MP4 file should be written to the current working directory

  Scenario: Recording filename includes session name and timestamp
    Given a session "myapp" has been running
    When I run "frameterm record export -s myapp"
    Then the filename should match the pattern "frameterm-myapp-<timestamp>.mp4"

  Scenario: Timestamps in recording correlate with wall clock
    Given a session "demo" has been running for 10 seconds
    When I export the recording
    Then the MP4 duration should be approximately 10 seconds

  Scenario: Recording captures terminal resize
    Given a session "demo" is running at 80x24
    When I run "frameterm resize -s demo 120 40"
    And I export the recording
    Then the MP4 should reflect the resize partway through

  Scenario: Export fails gracefully for empty session
    Given a session "empty" was just spawned with no activity
    When I run "frameterm record export -s empty"
    Then the command should fail with a JSON error
    And the error code should be "NO_FRAMES_RECORDED"
    And the error should include a suggestion
    And the process exit code should be non-zero

  Scenario: Export fails for nonexistent session
    When I run "frameterm record export -s nonexistent"
    Then the command should fail with a JSON error
    And the error code should be "SESSION_NOT_FOUND"
    And the process exit code should be non-zero

  Scenario: Recording continues after on-demand export
    Given a session "demo" is running
    When I run "frameterm record export -s demo"
    And I continue interacting with session "demo"
    And I run "frameterm record export -s demo" again
    Then the second MP4 should include all activity from the start

  Scenario: Disable recording for a session
    When I run "frameterm spawn --name ephemeral --no-record bash"
    Then no recording should be captured for session "ephemeral"
