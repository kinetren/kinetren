{
  "kinotrain": 1,
  "project": {
    "format_version": 1,
    "settings": {
      "fps": 30,
      "width": 1280,
      "height": 720,
      "sample_rate": 48000
    },
    "media": [
      {
        "id": 1,
        "path": "detail.mp4"
      },
      {
        "id": 2,
        "path": "green.mp4"
      },
      {
        "id": 3,
        "path": "detail-hi.mp4"
      }
    ],
    "clips": [
      {
        "id": 1,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 0,
        "duration": 360000,
        "source_in": 0,
        "media": 3,
        "gain_db": 0.0,
        "fx": []
      },
      {
        "id": 2,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 360000,
        "duration": 360000,
        "source_in": 0,
        "media": 3,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "geometry",
            "zoom": 2.0
          }
        ]
      },
      {
        "id": 3,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 720000,
        "duration": 360000,
        "source_in": 0,
        "media": 1,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "geometry",
            "zoom": 2.0
          }
        ]
      },
      {
        "id": 4,
        "link_group": null,
        "kind": "video",
        "lane": 0,
        "start": 1080000,
        "duration": 360000,
        "source_in": 0,
        "media": 3,
        "gain_db": 0.0,
        "fx": [
          {
            "kind": "vhs",
            "rgb_shift": 3,
            "bleed": 0.35
          },
          {
            "kind": "geometry",
            "zoom": 2.0
          }
        ]
      }
    ],
    "tracks": [
      {
        "id": 1,
        "kind": "video"
      },
      {
        "id": 2,
        "kind": "audio"
      }
    ],
    "overlap_layers": true
  }
}
