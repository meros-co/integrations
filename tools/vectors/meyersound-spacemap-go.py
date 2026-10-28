SG = "meyersound-spacemap-go"
# Spacemap Go general purpose OSC (User Guide, General purpose OSC commands):
# OSC 1.0 over UDP to port 38033, never answered.
binary(SG, "set_position", {"channel": 1, "x": 125, "y": 300}, osc("/channel/1/position", ("i", 125), ("i", 300)))   # guide example
binary(SG, "set_channel_level", {"channel": 2, "level_db": -6.0}, osc("/channel/2/level", ("f", -6.0)))
binary(SG, "set_channel_mute", {"channel": 32, "muted": True}, osc("/channel/32/mute", ("i", 1)))
binary(SG, "set_crossfade", {"channel": 1, "crossfade": -100}, osc("/channel/1/crossfade", ("i", -100)))
binary(SG, "set_spread", {"channel": 1, "spread": 14}, osc("/channel/1/spread", ("i", 14)))
binary(SG, "set_trajectory", {"channel": 3, "status": "play"}, osc("/channel/3/trajectory/status", ("s", "play")))
binary(SG, "set_mix_level", {"level_db": 10.0}, osc("/mix/level", ("f", 10.0)))
binary(SG, "set_mix_mute", {"muted": False}, osc("/mix/mute", ("i", 0)))
binary(SG, "set_system_level", {"level_db": -90.0}, osc("/system/level", ("f", -90.0)))
binary(SG, "recall_channel_snapshot", {"recall_id": 3003}, osc("/recall/channel", ("i", 3003)))
binary(SG, "recall_mix_snapshot", {"recall_id": 2002}, osc("/recall/mix", ("i", 2002)))
binary(SG, "recall_system_snapshot", {"recall_id": 1001}, osc("/recall/system", ("i", 1001)))
