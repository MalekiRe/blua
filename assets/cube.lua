local app = ...

local i = 0.0;

function my_system(query)
    i = i + 1.0
    for transform, _ in query:iter() do
        transform.translation.x = math.sin(i)
        --transform.scale.y = 1.0
        --transform.scale.y = math.sin(i * 0.01)
        print(transform)
    end
end

app:register_system(my_system, {
    { bevy_transform.components.transform.Transform.mut, cube.CubeMarker.mut}
 })