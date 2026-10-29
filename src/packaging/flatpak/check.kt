{
  "kinetren": 1,
  "project": {
    "format_version": 1,
    "settings": {
      "fps": 30,
      "width": 640,
      "height": 360,
      "sample_rate": 48000
    },
    "media": [
      {
        "id": 1,
        "path": "synth:ramp"
      },
      {
        "id": 2,
        "path": "synth:sine:440"
      }
    ],
    "clips": [
      {
        "id": 1,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 0,
        "duration": 180000,
        "source_in": 0,
        "media": 1,
        "gain_db": 0.0
      },
      {
        "id": 2,
        "link_group": null,
        "kind": "audio",
        "lane": 0,
        "start": 0,
        "duration": 180000,
        "source_in": 0,
        "media": 2,
        "gain_db": -12.0
      }
    ]
  }
}
