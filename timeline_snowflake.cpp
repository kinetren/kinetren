#include "timeline_snowflake.h"

#include <QDateTime>
#include <QRandomGenerator>

quint64 generateTimelineSnowflakeId()
{
    static quint64 seq = 0;
    const quint64 t = static_cast<quint64>(QDateTime::currentMSecsSinceEpoch()) & 0xFFFFFFFFFFFFULL;
    const quint64 r = QRandomGenerator::global()->generate64() & 0xFFULL;
    ++seq;
    return (t << 16) | (r << 8) | (seq & 0xFFULL);
}
