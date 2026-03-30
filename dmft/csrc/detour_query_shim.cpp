// Thin C shim around dtNavMeshQuery methods that bindgen can't wrap.
// Compiled by build.rs via the cc crate and linked into dmft.

#include "DetourNavMesh.h"
#include "DetourNavMeshQuery.h"

extern "C" {

dtStatus shim_dtNavMeshQuery_init(
    dtNavMeshQuery* query,
    const dtNavMesh* nav,
    int maxNodes)
{
    return query->init(nav, maxNodes);
}

dtStatus shim_dtNavMeshQuery_findNearestPoly(
    const dtNavMeshQuery* query,
    const float* center,
    const float* halfExtents,
    const dtQueryFilter* filter,
    dtPolyRef* nearestRef,
    float* nearestPt)
{
    return query->findNearestPoly(center, halfExtents, filter, nearestRef, nearestPt);
}

dtStatus shim_dtNavMeshQuery_findPath(
    const dtNavMeshQuery* query,
    dtPolyRef startRef,
    dtPolyRef endRef,
    const float* startPos,
    const float* endPos,
    const dtQueryFilter* filter,
    dtPolyRef* path,
    int* pathCount,
    int maxPath)
{
    return query->findPath(startRef, endRef, startPos, endPos, filter, path, pathCount, maxPath);
}

dtStatus shim_dtNavMeshQuery_findStraightPath(
    const dtNavMeshQuery* query,
    const float* startPos,
    const float* endPos,
    const dtPolyRef* path,
    int pathSize,
    float* straightPath,
    unsigned char* straightPathFlags,
    dtPolyRef* straightPathRefs,
    int* straightPathCount,
    int maxStraightPath,
    int options)
{
    return query->findStraightPath(
        startPos, endPos, path, pathSize,
        straightPath, straightPathFlags, straightPathRefs,
        straightPathCount, maxStraightPath, options);
}

} // extern "C"
