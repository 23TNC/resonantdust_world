// T=init
  for shard in all spacetime.event_shard:
    server.worker subscribes to shard.event_log select * from event_log where worker_reference == self.server_reference
  for shard in all spacetime.data_shard:
    server.worker subscribes to shard.state_hold select * from state_hold where worker_reference == self.server_reference

// T=0
  client.core sends request to queue event to server.edge
  server.edge validates the request
  server.edge makes a reducer call to spacetime.event_shard to queue event
  spacetime.event_shard creates a new row in event_log with the status "queued" at T=3

// T=1
  server.worker makes a reducer call to spacetime.event_shard with it's server_id to request work

  for work in spacetime.event_shard identifies pending work in event_log
    spacetime.event_shard.temp_event = work 
    if spacetime.event_shard.temp_event.status == "queued"
      spacetime.event_shard.temp_event.worker_reference = "queueing"
    elif spacetime.event_shard.temp_event.status == "queue_success"
      spacetime.event_shard.temp_event.worker_reference = "running"

  for event in server.worker.event_log (from its subscription)
    server.worker.temp_event = event
    event_reference is combine with event_shard's server_id to create an entity_reference
    if event.status == "queueing" && server.worker.temp_event.event_tic <= now.tic+2: // when queued at T=0 should execute at T=1
      if event.event_tic <= now.tic: 
        server.worker.temp_event.failed = true
      for target in event.targets
        if !server.worker.temp_event.failed:
          server.worker makes a reducer call to spacetime.data_shard to inform pending event for target
          spacetime.data_shard.state_log's row is created if not present
          spacetime.data_shard.temp_state_log = spacetime.data_shard.state_log's row for target at tic
          spacetime.data_shard.temp_state_log adds entity_reference to vec<u64> events
          for hold in spacetime.data_shard.state_hold where target_reference == spacetime.data_shard.temp_state_log.object_reference
            hold.dirty_count = length spacetime.data_shard.temp_state_log.object_reference.events
        else:
          server.worker makes a reducer call to spacetime.data_shard to inform failed event for target
          spacetime.data_shard.state_log removes entity_reference for row where target at tic if present
      if !server.worker.temp_event.failed:
        server.worker.temp_event.status = "queue_success"
      else:
        server.worker.temp_event.status = "queue_fail"
      server.worker makes a reducer call to spacetime.event_shard to update event to temp_event

// T=2
    if event.status == "running" && server.worker.temp_event.event_tic <= now.tic+1: // when queued at T=0 should execute at T=2
      for target in event.targets
        if server.worker.blocked[entity_reference] doesn't exist
          server.worker makes a reducer call to spacetime.data_shard asking to be assigned to work on target for entity_reference
          spacetime.data_shard creates a row in spacetime.data_shard.state_hold for target_reference and entity_reference if one does not exist
          spacetime.data_shard.hold = spacetime.data_shard.state_hold where target_reference and entity_reference
          spacetime.data_shard.hold is updated with worker_reference and dirty_count where dirty_count is the number of entity_reference in target.events
          spacetime.data_shard.state_hold.uid = hold
        if !server.worker.blocked[entity_reference]:
          server.worker.scratch = [][]
          event_success = true
          for action in event.actions
            event_success = server.worker handles action every target gets their own row in scratch
            if !event_success
              break
          if event_success
            write_success = true
            for target in event.targets
              write_success = server.worker makes a reducer call to spacetime.data_shard asking to update with scratch[target]
              if !write_success
                break
          if write_success
            server.worker makes a reducer call to spacetime.event_shard to update status to "complete"

// T=3
  spacetime.data_shard moves data from state log to state when promote is true
  spacetime.event_shard moves data from event log to event when promote is true