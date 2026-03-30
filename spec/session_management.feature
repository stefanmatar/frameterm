Feature: Session Management
  Sessions are isolated PTY environments, each running a terminal application.

  Scenario: Spawn a session with default name
    When I run "frameterm spawn htop"
    Then a session named "default" should be created
    And the session should be running "htop"

  Scenario: Spawn a session with a custom name
    When I run "frameterm spawn --name monitoring htop"
    Then a session named "monitoring" should be created

  Scenario: Spawn a session with a working directory
    When I run "frameterm spawn --cwd /tmp --name tmp-session ls"
    Then the session "tmp-session" should have working directory "/tmp"

  Scenario: Spawn multiple named sessions
    When I run "frameterm spawn --name app1 htop"
    And I run "frameterm spawn --name app2 vim"
    Then there should be 2 active sessions

  Scenario: List active sessions
    Given a session "editor" is running
    And a session "monitor" is running
    When I run "frameterm list-sessions"
    Then the output should list "editor"
    And the output should list "monitor"

  Scenario: Kill a specific session
    Given a session "editor" is running
    And a session "monitor" is running
    When I run "frameterm kill -s editor"
    Then session "editor" should not exist
    And session "monitor" should still be running

  Scenario: Kill the default session
    Given a session "default" is running
    When I run "frameterm kill"
    Then session "default" should not exist

  Scenario: Session is removed when its process exits
    Given a session "short-lived" is running "echo done"
    When the process in "short-lived" exits
    Then "frameterm list-sessions" should not include "short-lived"

  Scenario: Spawn fails with duplicate session name
    Given a session "myapp" is running
    When I run "frameterm spawn --name myapp vim"
    Then the command should fail with a JSON error
    And the error code should be "SESSION_ALREADY_EXISTS"
    And the error should include a suggestion
    And the process exit code should be non-zero

  Scenario: Spawn fails with invalid command
    When I run "frameterm spawn nonexistent-binary-xyz"
    Then the command should fail with a JSON error
    And the error code should be "SPAWN_FAILED"
    And the message should indicate the command was not found
    And the process exit code should be non-zero

  Scenario: Kill fails for nonexistent session
    When I run "frameterm kill -s nonexistent"
    Then the command should fail with a JSON error
    And the error code should be "SESSION_NOT_FOUND"
    And the suggestion should mention "frameterm list-sessions"
    And the process exit code should be non-zero

  Scenario: Stop terminates all sessions
    Given a session "app1" is running
    And a session "app2" is running
    When I run "frameterm stop"
    Then all sessions should be terminated
    And "frameterm list-sessions" should return an empty list

  Scenario: Default session name from environment variable
    Given the environment variable "FRAMETERM_SESSION" is set to "custom"
    When I run "frameterm spawn bash" without --name
    Then the session should be named "custom"
