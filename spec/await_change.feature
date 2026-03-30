Feature: Await Screen Changes
  Instead of guessing sleep durations, agents can block until the screen changes.

  Background:
    Given a session "app" is running

  Scenario: Block until screen changes
    Given I have a content_hash from a previous snapshot
    When I send input that causes the screen to change
    And I run "frameterm snapshot -s app --await-change <hash>"
    Then the command should return once the screen content differs from <hash>
    And the returned snapshot should have a different content_hash

  Scenario: Await change with settle time
    Given I have a content_hash from a previous snapshot
    When I send input that causes progressive rendering
    And I run "frameterm snapshot -s app --await-change <hash> --settle 100"
    Then the command should wait for the screen to be stable for 100ms after the initial change
    And then return the final snapshot

  Scenario: Await change with timeout
    Given I have a content_hash from a previous snapshot
    When the screen does not change
    And I run "frameterm snapshot -s app --await-change <hash> --timeout 2000"
    Then the command should fail with a JSON error after 2000ms
    And the error code should be "AWAIT_TIMEOUT"
    And the process exit code should be non-zero

  Scenario: Default timeout for await-change
    Given I have a content_hash from a previous snapshot
    When the screen does not change
    And I run "frameterm snapshot -s app --await-change <hash>"
    Then the command should fail with a JSON error after 30 seconds
    And the error code should be "AWAIT_TIMEOUT"
    And the process exit code should be non-zero

  Scenario: Await change for streaming AI responses
    Given a session "ai" is running an application that streams text
    And I have a content_hash from before the stream started
    When I run "frameterm snapshot -s ai --await-change <hash> --settle 3000 --timeout 60000"
    Then the command should wait until the stream finishes (3s of no changes)
    And return the final snapshot
