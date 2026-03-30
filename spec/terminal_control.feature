Feature: Terminal Control
  Agents can resize the terminal and control session dimensions.

  Background:
    Given a session "app" is running

  Scenario: Resize terminal
    When I run "frameterm resize -s app 120 40"
    Then the session terminal should be 120 columns by 40 rows

  Scenario: Resize is reflected in snapshots
    When I run "frameterm resize -s app 100 30"
    And I take a snapshot
    Then the snapshot size should show cols=100 and rows=30

  Scenario: Default terminal size
    When I run "frameterm spawn --name fresh bash"
    And I take a snapshot of "fresh"
    Then the snapshot size should show cols=80 and rows=24

  Scenario: Custom initial terminal size
    When I run "frameterm spawn --name custom --cols 120 --rows 40 bash"
    And I take a snapshot of "custom"
    Then the snapshot size should show cols=120 and rows=40
