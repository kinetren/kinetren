#ifndef TIMELINE_SNOWFLAKE_H
#define TIMELINE_SNOWFLAKE_H

#include <QtGlobal>

// Monotonic-ish 64-bit id for stable project references (clips / morphers).
quint64 generateTimelineSnowflakeId();

#endif
